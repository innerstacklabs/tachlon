//! The comparison bars (gs#703): which visible bars get a thin bar under them at the baseline
//! dates, how much work that takes, and where it sits in the draw list.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{DAY, Instant, TaskDates};
use std::cell::Cell;
use timeline::scene::{self, Rect, Scene, span_extent};
use timeline::summary::Summaries;
use timeline::viewport::{AXIS_HEIGHT, ROW_HEIGHT};
use timeline::{Theme, Viewport};

const THEME: Theme = Theme::light();

/// Task i is on row i; a day is 20 px; the canvas is 1000 px wide and shows `rows` rows.
fn view(rows: usize, tasks: usize) -> Viewport {
    let mut v = Viewport::new(1000.0, AXIS_HEIGHT + rows as f64 * ROW_HEIGHT, tasks);
    v.px_per_minute = 20.0 / DAY as f64;
    v.t0 = 0.0;
    v
}

fn dates(n: usize) -> Vec<TaskDates> {
    (0..n)
        .map(|i| {
            let day = (i as i64) % 20;
            TaskDates {
                es: day * DAY,
                ef: day * DAY + 2 * DAY,
                ls: day * DAY,
                lf: day * DAY + 2 * DAY,
                float: 0,
                finished: false,
            }
        })
        .collect()
}

fn order(n: usize) -> Vec<u32> {
    (0..n as u32).collect()
}

fn bars(v: &Viewport, n: usize, summaries: &Summaries) -> Scene {
    scene::build(v, 1.0, &order(n), &dates(n), summaries, None, None, &THEME)
}

fn add(
    s: &mut Scene,
    v: &Viewport,
    sums: &Summaries,
    n: usize,
    of: impl Fn(u32) -> Option<(Instant, Instant)>,
) {
    scene::add_comparison_bars(s, v, 1.0, sums, &order(n), of, (0, THEME.baseline));
}

fn comparison(s: &Scene) -> Vec<Rect> {
    s.rects
        .iter()
        .filter(|r| r.color == THEME.baseline)
        .copied()
        .collect()
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "`add` takes a function that returns an Option, since a task can have no baseline, and this one gives every task a baseline"
)]
fn planned(_: u32) -> Option<(Instant, Instant)> {
    Some((5 * DAY, 7 * DAY))
}

#[test]
fn only_a_baselined_visible_bar_gets_a_comparison_bar() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    // 1, 2: drawn. 3: no entry. 30: a row off the canvas.
    add(&mut s, &v, &sums, 40, |t| {
        [1, 2, 30].contains(&t).then_some((5 * DAY, 7 * DAY))
    });
    let got = comparison(&s);
    let (x0, x1) = span_extent(&v, 5 * DAY, 7 * DAY);
    let want: Vec<([f32; 2], [f32; 2])> = [1, 2]
        .iter()
        .map(|&r| {
            // The bar is 12 px in the 24 px row, 6 px from the top; the strip is 1 px under it.
            let top = AXIS_HEIGHT + r as f64 * ROW_HEIGHT + 6.0 + 12.0 + 1.0;
            ([x0 as f32, top as f32], [(x1 - x0) as f32, 3.0])
        })
        .collect();
    let have: Vec<_> = got.iter().map(|r| (r.pos, r.size)).collect();
    assert_eq!(have, want);
    assert!(got.iter().all(|r| r.stroke == 0.0));
}

#[test]
fn a_bar_under_the_axis_band_gets_none() {
    let mut v = view(20, 40);
    v.scroll_by(10.0);
    let sums = Summaries::new(40);
    let mut s = bars(&v, 40, &sums);
    add(&mut s, &v, &sums, 40, planned);
    let tops: Vec<f64> = comparison(&s).iter().map(|r| f64::from(r.pos[1])).collect();
    // Row 0's bar is cut by the band, so it gets none (its strip would sit at AXIS_HEIGHT + 9);
    // rows 1 to 20 are whole.
    assert_eq!(tops.len(), 20);
    assert!(tops.iter().all(|&t| t > AXIS_HEIGHT + 9.0));
}

#[test]
fn a_summary_and_a_task_with_no_entry_get_none() {
    let mut sums = Summaries::new(40);
    sums.set(5, Some((0, 3 * DAY)));
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    add(&mut s, &v, &sums, 40, |t| {
        (t != 7).then_some((5 * DAY, 7 * DAY))
    });
    let rows: Vec<usize> = comparison(&s)
        .iter()
        .map(|r| ((f64::from(r.pos[1]) - AXIS_HEIGHT) / ROW_HEIGHT) as usize)
        .collect();
    assert!(!rows.contains(&5), "a summary has no comparison bar");
    assert!(!rows.contains(&7), "no entry, no comparison bar");
    assert_eq!(rows.len(), 18);
}

#[test]
fn the_work_follows_the_visible_rows_not_the_tasks() {
    let n = 50_000;
    let v = view(30, n);
    let sums = Summaries::new(n);
    let mut s = bars(&v, n, &sums);
    let calls = Cell::new(0usize);
    add(&mut s, &v, &sums, n, |t| {
        calls.set(calls.get() + 1);
        planned(t)
    });
    assert!(
        calls.get() <= v.visible_rows().len(),
        "{} lookups",
        calls.get()
    );
    assert!(comparison(&s).len() <= v.visible_rows().len());
}

#[test]
fn bars_go_under_the_bars_and_the_bar_indexes_follow() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    let before = s.clone();
    add(&mut s, &v, &sums, 40, planned);
    let added = comparison(&s).len();
    assert!(added > 0);
    assert_eq!(s.rects.len(), before.rects.len() + added);
    let first = s
        .rects
        .iter()
        .position(|r| r.color == THEME.baseline)
        .unwrap();
    assert_eq!(first, before.bar_start);
    for (old, new) in before.bar_rects.iter().zip(&s.bar_rects) {
        assert_eq!(new.1, old.1 + added);
        assert_eq!(s.rects[new.1], before.rects[old.1]);
    }
}

#[test]
fn no_baseline_adds_nothing() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    let before = s.clone();
    add(&mut s, &v, &sums, 40, |_| None);
    assert_eq!(s, before);
}

fn a_later_slot_in_its_own_color(v: &Viewport) -> (Scene, Rect) {
    let sums = Summaries::new(40);
    let mut s = bars(v, 40, &sums);
    let teal = [0.0, 0.5, 0.5, 1.0];
    let of = |t| (t == 1).then_some((5 * DAY, 7 * DAY));
    scene::add_comparison_bars(&mut s, v, 1.0, &sums, &order(40), of, (1, teal));
    let got: Vec<_> = s.rects.iter().filter(|r| r.color == teal).collect();
    assert_eq!(got.len(), 1);
    let strip = *got[0];
    (s, strip)
}

#[test]
fn in_a_24_px_row_a_later_slot_lies_inside_its_row_close_under_the_bar() {
    let v = view(20, 40);
    let (s, strip) = a_later_slot_in_its_own_color(&v);
    let row = AXIS_HEIGHT + ROW_HEIGHT;
    // The 12 px bar is 6 px from the row's top; the strip is 3 px tall.
    let (bar_bottom, top) = (row + 6.0 + 12.0, f64::from(strip.pos[1]));
    let (gap, below) = (top - bar_bottom, row + ROW_HEIGHT - (top + 3.0));
    assert!(
        below >= 2.0,
        "{below} px of the row's background under the strip"
    );
    assert!(
        gap < below,
        "gap to the bar {gap} against {below} to the row's bottom edge"
    );
    assert_eq!(
        s.rects[s.bar_rects[1].1].pos[1],
        (row + 6.0) as f32,
        "the bar did not move"
    );
}

#[test]
fn in_a_taller_row_a_later_slot_still_sits_one_strip_lower() {
    let mut v = view(20, 40);
    v.row_height = 48.0;
    let (_, strip) = a_later_slot_in_its_own_color(&v);
    let row = AXIS_HEIGHT + 48.0;
    // The 24 px bar is 12 px from the row's top; slot 0 is 1 px under it and 3 px tall, so slot 1 starts 4 px lower.
    assert_eq!(f64::from(strip.pos[1]), row + 12.0 + 24.0 + 1.0 + 4.0);
}
