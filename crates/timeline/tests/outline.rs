//! The move outline (gs#428, story S4 of #309): which moved tasks get a dashed ring at their old
//! place, how many, and where it sits in the draw list.

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
use std::mem::{offset_of, size_of};
use timeline::scene::{self, OUTLINE_CAP, Rect, STROKE_PX, Scene, span_extent};
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

/// The scene of the bars alone.
fn bars(v: &Viewport, n: usize, summaries: &Summaries) -> Scene {
    let order: Vec<u32> = (0..n as u32).collect();
    scene::build(v, 1.0, &order, &dates(n), summaries, None, None, &THEME)
}

type Was = Vec<(u32, Instant, Instant)>;

/// Add the outlines of `was` at `alpha`; rows are the task ids except for those in `hidden`.
fn outline(s: &mut Scene, v: &Viewport, sums: &Summaries, was: &Was, alpha: f64, hidden: &[u32]) {
    let row_of = |t: u32| (!hidden.contains(&t)).then_some(t as usize);
    scene::add_move_outlines(s, v, 1.0, sums, row_of, was.iter().copied(), alpha, &THEME);
}

fn rings(s: &Scene) -> Vec<Rect> {
    s.rects.iter().filter(|r| r.stroke > 0.0).copied().collect()
}

fn was_of(tasks: &[u32]) -> Was {
    tasks.iter().map(|&t| (t, 5 * DAY, 7 * DAY)).collect()
}

#[test]
fn only_a_moved_visible_bar_gets_a_ring() {
    let mut sums = Summaries::new(40);
    sums.set(5, Some((0, 3 * DAY)));
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    // 1, 2: drawn. 5: a summary. 7: hidden. 30: a row off the canvas.
    outline(&mut s, &v, &sums, &was_of(&[1, 2, 5, 7, 30]), 1.0, &[7]);
    let tops: Vec<f32> = rings(&s).iter().map(|r| r.pos[1]).collect();
    let want: Vec<f32> = [1, 2]
        .iter()
        .map(|&r| (AXIS_HEIGHT + r as f64 * ROW_HEIGHT + 6.0) as f32)
        .collect();
    assert_eq!(tops, want);
}

#[test]
fn a_bar_under_the_axis_band_gets_no_ring() {
    let mut v = view(20, 40);
    v.scroll_by(10.0);
    let sums = Summaries::new(40);
    let mut s = bars(&v, 40, &sums);
    // Row 0 is cut by the band (its bar starts above the band's lower edge); row 1 is whole.
    outline(&mut s, &v, &sums, &was_of(&[0, 1]), 1.0, &[]);
    assert_eq!(rings(&s).len(), 1);
}

#[test]
fn a_tall_canvas_keeps_the_four_hundred_nearest_the_centre() {
    let n = OUTLINE_CAP + 50;
    let v = view(n, n);
    let sums = Summaries::new(n);
    let mut s = bars(&v, n, &sums);
    let all: Vec<u32> = (0..n as u32).collect();
    outline(&mut s, &v, &sums, &was_of(&all), 1.0, &[]);
    let rows: Vec<usize> = rings(&s)
        .iter()
        .map(|r| ((f64::from(r.pos[1]) - AXIS_HEIGHT - 6.0) / ROW_HEIGHT).round() as usize)
        .collect();
    // The centre falls between rows 224 and 225; the same x for all, so the nearest 400 rows are
    // the 200 on each side.
    assert_eq!(rows, (25..425).collect::<Vec<_>>());
}

#[test]
fn a_clipped_ring_keeps_its_dash_phase_while_the_view_scrolls() {
    let sums = Summaries::new(40);
    let period = 5.0 * STROKE_PX;
    for start_px in [-1000.3, -1003.9, -1010.0, -1013.7] {
        let mut v = view(20, 40);
        // The bar starts at `start_px` and runs 1300 px.
        let start = start_px / v.px_per_minute;
        v.t0 = 0.0;
        let was: Was = vec![(
            1,
            start as Instant,
            (start + 1300.0 / v.px_per_minute) as Instant,
        )];
        let mut s = bars(&v, 40, &sums);
        outline(&mut s, &v, &sums, &was, 1.0, &[]);
        let ring = rings(&s)[0];
        let real = v.x_of(was[0].1 as f64);
        let periods = (f64::from(ring.pos[0]) - real) / period;
        let drift = (periods - periods.round()).abs() * period;
        assert!(
            drift <= 0.5 + 1e-6,
            "start {real}: drawn at {}, {drift} px off a whole period",
            ring.pos[0]
        );
        assert!(ring.pos[0] <= -1.0, "the clipped start is on the canvas");
    }
}

#[test]
fn the_ring_sits_at_the_old_extent_and_a_milestone_is_a_square() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    let was: Was = vec![(1, 5 * DAY, 7 * DAY), (2, 9 * DAY, 9 * DAY)];
    outline(&mut s, &v, &sums, &was, 1.0, &[]);
    let found = rings(&s);
    assert_eq!(found.len(), 2);
    for (ring, &(_, es, ef)) in found.iter().zip(&was) {
        let (x0, x1) = span_extent(&v, es, ef);
        assert_eq!(f64::from(ring.pos[0]), x0.round());
        assert_eq!(f64::from(ring.pos[0] + ring.size[0]), x1.round());
    }
    assert_eq!(
        found[1].size[0], found[1].size[1],
        "a milestone is the square"
    );
}

#[test]
fn a_ring_has_a_stroke_and_nothing_else_does() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut s = bars(&v, 40, &sums);
    assert!(s.rects.iter().all(|r| r.stroke == 0.0));
    let before = s.rects.len();
    scene::add_move_outlines(
        &mut s,
        &v,
        2.0,
        &sums,
        |t| Some(t as usize),
        was_of(&[1, 2]).into_iter(),
        1.0,
        &THEME,
    );
    let found = rings(&s);
    assert_eq!(found.len(), 2);
    assert_eq!(s.rects.len(), before + 2, "a ring is one rect");
    assert!(
        found
            .iter()
            .all(|r| (f64::from(r.stroke) - STROKE_PX * 2.0).abs() < 1e-6)
    );
}

#[test]
fn alpha_scales_the_colour_and_zero_draws_nothing() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let mut full = bars(&v, 40, &sums);
    outline(&mut full, &v, &sums, &was_of(&[1]), 1.0, &[]);
    let mut half = bars(&v, 40, &sums);
    outline(&mut half, &v, &sums, &was_of(&[1]), 0.5, &[]);
    let mut none = bars(&v, 40, &sums);
    let untouched = none.clone();
    outline(&mut none, &v, &sums, &was_of(&[1]), 0.0, &[]);
    let (a, b) = (rings(&full)[0].color, rings(&half)[0].color);
    assert_eq!(a[..3], THEME.sel_edge[..3]);
    assert!((a[3] - THEME.sel_edge[3] * 0.6).abs() < 1e-6);
    assert!((b[3] - THEME.sel_edge[3] * 0.3).abs() < 1e-6);
    assert_eq!(none, untouched);
}

#[test]
fn rings_go_under_the_bars_and_the_bar_indexes_follow() {
    let sums = Summaries::new(40);
    let v = view(20, 40);
    let before = bars(&v, 40, &sums);
    let mut s = before.clone();
    outline(&mut s, &v, &sums, &was_of(&[1, 2, 3]), 1.0, &[]);
    let first_bar = s.bar_rects.iter().map(|&(_, i)| i).min().unwrap();
    let last_ring = s.rects.iter().rposition(|r| r.stroke > 0.0).unwrap();
    assert!(last_ring < first_bar, "a ring is above a bar");
    for (now, was) in s.bar_rects.iter().zip(&before.bar_rects) {
        assert_eq!(s.rects[now.1], before.rects[was.1], "task {}", now.0);
    }
    for (now, was) in s.bar_progress.iter().zip(&before.bar_progress) {
        assert_eq!(s.rects[now.1], before.rects[was.1]);
    }
}

/// The scene of `bars` with the ghost of `was` added.
fn with_ghosts(v: &Viewport, sums: &Summaries, was: &[(u32, Instant, Instant)]) -> Scene {
    let mut s = bars(v, 40, sums);
    let row_of = |t: u32| Some(t as usize);
    scene::add_ghost_outlines(
        &mut s,
        v,
        1.0,
        sums,
        row_of,
        was.iter().copied(),
        1.0,
        &THEME,
    );
    s
}

#[test]
fn a_ghost_is_drawn_after_its_bar() {
    let (sums, v) = (Summaries::new(40), view(20, 40));
    // Task 1 has its bar on days 1 to 3; the ghost of a shortened task lies inside it.
    let s = with_ghosts(&v, &sums, &[(1, DAY, 2 * DAY)]);
    let ring = s.rects.iter().rposition(|r| r.stroke > 0.0).unwrap();
    let last_bar = s
        .bar_rects
        .iter()
        .chain(&s.bar_progress)
        .map(|&(_, i)| i)
        .max()
        .unwrap();
    assert!(ring > last_bar, "a ghost is under a bar");
}

#[test]
fn a_ghost_is_wider_and_stronger_than_a_moved_outline() {
    let (sums, v) = (Summaries::new(40), view(20, 40));
    let mut moved = bars(&v, 40, &sums);
    outline(&mut moved, &v, &sums, &was_of(&[1]), 1.0, &[]);
    let (m, g) = (
        rings(&moved)[0],
        rings(&with_ghosts(&v, &sums, &was_of(&[1])))[0],
    );
    assert!(g.stroke > m.stroke, "{} vs {}", g.stroke, m.stroke);
    assert!(g.color[3] > m.color[3], "{} vs {}", g.color[3], m.color[3]);
    assert_eq!(
        g.color[..3],
        THEME.sel_edge[..3],
        "a ghost is the edge colour"
    );
}

#[test]
fn a_ghost_is_thinner_and_fainter_than_the_first_one_was() {
    let (sums, v) = (Summaries::new(40), view(20, 40));
    let g = rings(&with_ghosts(&v, &sums, &was_of(&[1])))[0];
    // #1788 drew it 2.5 px wide at the full edge alpha, which shouted at the default zoom.
    assert!(g.stroke < 2.5, "{} px", g.stroke);
    assert!(g.color[3] < THEME.sel_edge[3], "{}", g.color[3]);
}

#[test]
fn a_milestones_ghost_and_outline_are_diamonds_and_a_tasks_are_not() {
    let (sums, v) = (Summaries::new(40), view(20, 40));
    let milestone = vec![(1, 5 * DAY, 5 * DAY)];
    let g = rings(&with_ghosts(&v, &sums, &milestone))[0];
    assert!(
        g.radius < 0.0,
        "a milestone's ghost has radius {}",
        g.radius
    );
    assert_eq!(g.size[0], g.size[1], "a diamond sits in a square");
    let mut moved = bars(&v, 40, &sums);
    outline(&mut moved, &v, &sums, &milestone, 1.0, &[]);
    assert!(rings(&moved)[0].radius < 0.0);
    let task = rings(&with_ghosts(&v, &sums, &was_of(&[1])))[0];
    assert!(
        task.radius >= 0.0,
        "a task's ghost has radius {}",
        task.radius
    );
}

#[test]
fn a_milestones_ghost_at_the_canvas_edge_is_still_a_whole_diamond() {
    let (sums, v) = (Summaries::new(40), view(20, 40));
    // Day 50 is x = 1000, the canvas width: half of the diamond lies past the edge.
    let g = rings(&with_ghosts(&v, &sums, &[(1, 50 * DAY, 50 * DAY)]))[0];
    assert_eq!(g.size[0], g.size[1], "a diamond sits in a square");
}

#[test]
fn the_rect_is_forty_bytes_and_the_shader_reads_the_stroke_at_36() {
    assert_eq!(size_of::<Rect>(), 40);
    assert_eq!(offset_of!(Rect, stroke), 36);
}
