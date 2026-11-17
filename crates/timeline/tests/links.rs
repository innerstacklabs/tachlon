//! The link-range filter: which dependency arrows the visible range draws
//! (plan D4: "visible range only; an arrow is drawn only where an end of the
//! link, or its segment, crosses the view").

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{DAY, Dependency, LinkType, TaskDates, TaskId};
use std::collections::BTreeSet;
use timeline::scene::{self, LinkIndex, Scene};
use timeline::viewport::{AXIS_HEIGHT, ROW_HEIGHT};
use timeline::{Theme, Viewport};

/// Tall enough for exactly 17 rows under the axis, whatever the row height.
const VIEW_H: f64 = AXIS_HEIGHT + 17.0 * ROW_HEIGHT;

const THEME: Theme = Theme::light();

const N: usize = 1000;
/// Rows 500..517 are visible in `view`.
const FIRST: usize = 500;

/// Task i sits on row i. Its bar is the day i by default; `moved` puts a
/// task on another day, so a link can have an end far off the canvas.
fn dates(moved: &[(usize, i64)]) -> Vec<TaskDates> {
    (0..N)
        .map(|i| {
            let day = moved.iter().find(|m| m.0 == i).map_or(i as i64, |m| m.1);
            TaskDates {
                es: day * DAY,
                ef: day * DAY + DAY / 2,
                ls: day * DAY,
                lf: day * DAY + DAY / 2,
                float: 0,
                finished: false,
            }
        })
        .collect()
}

fn dep(pred: u32, succ: u32) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

/// 1000 px wide, rows `FIRST..FIRST + 17` in view, days 495 to 525 across.
fn view() -> Viewport {
    let mut v = Viewport::new(1000.0, VIEW_H, N);
    v.px_per_minute = 1000.0 / (30.0 * DAY as f64);
    v.t0 = 495.0 * DAY as f64;
    v.scroll_to_row(FIRST);
    assert_eq!(v.visible_rows(), FIRST..FIRST + 17);
    v
}

fn pairs(index: &LinkIndex, v: &Viewport, d: &[TaskDates]) -> BTreeSet<(u32, u32)> {
    index
        .visible(v, d)
        .iter()
        .map(|p| (p.link.pred, p.link.succ))
        .collect()
}

#[test]
fn a_collapsed_order_shorter_than_the_task_count_keeps_visible_links() {
    let dates: Vec<TaskDates> = (0..6)
        .map(|day| TaskDates {
            es: day * DAY,
            ef: day * DAY + DAY / 2,
            ls: day * DAY,
            lf: day * DAY + DAY / 2,
            float: 0,
            finished: false,
        })
        .collect();
    let order = [0, 1, 5];
    let deps = [dep(0, 1), dep(1, 5), dep(1, 3)];
    let index = LinkIndex::new(&deps, |_| true, &order);
    assert_eq!(index.len(), 2);

    let mut viewport = Viewport::new(600.0, 200.0, order.len());
    viewport.fit(0.0, 6.0 * DAY as f64);
    let links: Vec<_> = index
        .visible(&viewport, &dates)
        .into_iter()
        .map(|placed| {
            (
                placed.link.pred,
                placed.link.succ,
                placed.link.pred_row,
                placed.link.succ_row,
            )
        })
        .collect();
    assert_eq!(links, [(0, 1, 0, 1), (1, 5, 1, 2)]);
    assert!(
        links
            .iter()
            .all(|&(pred, succ, _, _)| pred != 3 && succ != 3)
    );
}

#[test]
fn an_empty_order_builds_an_empty_link_index() {
    let index = LinkIndex::new(&[dep(0, 1)], |_| true, &[]);
    assert!(index.is_empty());
}

#[test]
fn a_hidden_end_above_the_largest_visible_id_is_left_out() {
    let index = LinkIndex::new(&[dep(1, 4)], |_| true, &[0, 1]);
    assert!(index.is_empty());
}

#[test]
fn only_links_that_meet_the_view_are_kept() {
    // Tasks 508, 509, 510, 410 sit on days far left of the canvas and 511,
    // 512, 513 far right of it; 600 and 800 sit on days in view.
    let d = dates(&[
        (508, 100),
        (509, 100),
        (510, 100),
        (511, 900),
        (512, 900),
        (513, 900),
        (410, 100),
        (600, 515),
        (800, 512),
    ]);
    let order: Vec<u32> = (0..N as u32).collect();
    let links: &[(u32, u32)] = &[
        (502, 503), // both ends in view
        (10, 11),   // rows far above
        (900, 901), // rows far below
        (495, 499), // rows just above the window
        (499, 500), // last row above to first row in view
        (516, 520), // first end on the last visible row
        (517, 520), // rows just below the window
        (400, 600), // rows above and below the window, vertical run in view
        (100, 800), // the same as a long link
        (410, 610), // rows above and below the window, path off the canvas
        (200, 700), // the same, with the ends on days far from the view
        (508, 509), // in the row window, both ends left of the canvas
        (512, 513), // in the row window, both ends right of the canvas
        (510, 511), // in the row window, the run between rows crosses the canvas
    ];
    let deps: Vec<Dependency> = links.iter().map(|&(p, s)| dep(p, s)).collect();
    let index = LinkIndex::new(&deps, |_| true, &order);
    assert_eq!(index.len(), links.len());
    let kept = pairs(&index, &view(), &d);
    let want: BTreeSet<(u32, u32)> = [
        (502, 503),
        (499, 500),
        (516, 520),
        (400, 600),
        (100, 800),
        (510, 511),
    ]
    .into();
    assert_eq!(kept, want);
}

#[test]
fn the_search_window_reaches_exactly_as_far_up_as_a_short_link_can_span() {
    // Both links end on the first visible row. The first spans 256 rows (the
    // longest "short" link, found by binary search), the second 257 (the
    // shortest "long" link, found by the scan); a window one row too narrow
    // loses the first.
    let d = dates(&[]);
    let order: Vec<u32> = (0..N as u32).collect();
    let deps = vec![
        dep(FIRST as u32 - 256, FIRST as u32),
        dep(FIRST as u32 - 257, FIRST as u32),
    ];
    let index = LinkIndex::new(&deps, |_| true, &order);
    let want: BTreeSet<(u32, u32)> = [(244, 500), (243, 500)].into();
    assert_eq!(pairs(&index, &view(), &d), want);
}

#[test]
fn a_chain_of_fifty_thousand_links_keeps_the_visible_ones() {
    let n = 50_000;
    let order: Vec<u32> = (0..n as u32).collect();
    let deps: Vec<Dependency> = (0..n as u32 - 1).map(|i| dep(i, i + 1)).collect();
    let d: Vec<TaskDates> = (0..n as i64)
        .map(|i| TaskDates {
            es: i * DAY,
            ef: i * DAY + DAY / 2,
            ls: i * DAY,
            lf: i * DAY + DAY / 2,
            float: 0,
            finished: false,
        })
        .collect();
    let index = LinkIndex::new(&deps, |_| true, &order);
    assert_eq!(index.len(), n - 1);
    let mut v = Viewport::new(1000.0, VIEW_H, n);
    v.px_per_minute = 1000.0 / (30.0 * DAY as f64);
    v.t0 = 495.0 * DAY as f64;
    v.scroll_to_row(FIRST);
    // Links i -> i + 1 with i + 1 >= 500 and i < 517.
    assert_eq!(index.visible(&v, &d).len(), 18);
}

#[test]
fn arrows_are_drawn_for_the_kept_links_and_stay_inside_the_view() {
    let d = dates(&[(508, 100), (509, 100)]);
    let order: Vec<u32> = (0..N as u32).collect();
    // 490 -> 505 comes from above the window: its vertical run is cut at the
    // axis band, not drawn over it.
    let deps = vec![
        dep(502, 503),
        dep(506, 507),
        dep(508, 509),
        dep(10, 11),
        dep(490, 505),
    ];
    let index = LinkIndex::new(&deps, |_| true, &order);
    let v = view();
    let mut s = Scene::default();
    scene::add_arrows(&mut s, &v, 2.0, &d, &index, &THEME);
    assert_eq!(s.arrows, 3);
    let arrows: Vec<_> = s.rects.iter().filter(|r| r.color == THEME.link).collect();
    assert!(arrows.len() >= 3 * 4, "{} arrow rects", arrows.len());
    assert_eq!(arrows.len(), s.rects.len(), "nothing else is added");
    let (width_px, height_px, axis) = (
        v.width as f32 * 2.0,
        v.height as f32 * 2.0,
        v.axis_height as f32 * 2.0,
    );
    for r in arrows {
        let (x1, y1) = (r.pos[0] + r.size[0], r.pos[1] + r.size[1]);
        assert!(r.pos[0] >= -4.0 && x1 <= width_px + 4.0, "{r:?}");
        assert!(r.pos[1] >= axis && y1 <= height_px, "{r:?}");
    }
}

#[test]
fn a_zero_lag_link_enters_the_successor_from_outside_its_bar() {
    // Finish-to-start with no lag: the successor starts where the
    // predecessor finishes, the common case. The arrowhead must not sit
    // under the successor's bar.
    let half = DAY / 2;
    let task = |es, ef| TaskDates {
        es,
        ef,
        ls: es,
        lf: ef,
        float: 0,
        finished: false,
    };
    let d = vec![task(0, half), task(half, DAY)];
    let order = vec![0, 1];
    let index = LinkIndex::new(&[dep(0, 1)], |_| true, &order);
    let mut v = Viewport::new(600.0, 200.0, 2);
    v.fit(0.0, DAY as f64);
    let mut s = scene::build(
        &v,
        1.0,
        &order,
        &d,
        &timeline::Summaries::default(),
        None,
        None,
        &THEME,
    );
    scene::add_arrows(&mut s, &v, 1.0, &d, &index, &THEME);
    assert_eq!(s.arrows, 1);
    let bars: Vec<&scene::Rect> = s
        .rects
        .iter()
        .filter(|r| r.color == THEME.crit_soft)
        .collect();
    assert_eq!(bars.len(), 2);
    let succ = bars[1];
    let (top, bottom) = (succ.pos[1], succ.pos[1] + succ.size[1]);
    let into: Vec<&scene::Rect> = s
        .rects
        .iter()
        .filter(|r| r.color == THEME.link && r.pos[1] < bottom && r.pos[1] + r.size[1] > top)
        .collect();
    assert!(!into.is_empty(), "the arrow reaches the successor's row");
    for r in into {
        assert!(
            r.pos[0] + r.size[0] <= succ.pos[0] + 1.0,
            "{r:?} is inside {succ:?}"
        );
    }
}

#[test]
fn every_kind_enters_the_successor_from_outside_its_bar_in_both_directions() {
    let half = DAY / 2;
    let task = |es, ef| TaskDates {
        es,
        ef,
        ls: es,
        lf: ef,
        float: 0,
        finished: false,
    };
    // Task 0 is the predecessor, task 1 the successor; they overlap in time,
    // so every kind has room to be wrong. Tasks 2 to 5 fill the other rows.
    let mut d = vec![task(0, half), task(half / 2, DAY)];
    d.extend((2..6).map(|_| task(10 * DAY, 11 * DAY)));
    for kind in [LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF] {
        for up in [false, true] {
            // Rows 2 and 3 hold the pair; `up` puts the successor above.
            let mut order: Vec<u32> = vec![2, 3, 0, 1, 4, 5];
            if up {
                order.swap(2, 3);
            }
            let link = Dependency {
                pred: TaskId(0),
                succ: TaskId(1),
                kind,
                lag: 0,
            };
            let index = LinkIndex::new(&[link], |_| true, &order);
            let mut v = Viewport::new(600.0, 300.0, 6);
            v.fit(0.0, DAY as f64);
            let mut s = Scene::default();
            scene::add_arrows(&mut s, &v, 1.0, &d, &index, &THEME);
            let what = format!("{kind:?}, successor {}", if up { "above" } else { "below" });
            assert_eq!(s.arrows, 1, "{what}");
            let (x0, x1) = scene::bar_extent(&v, &d[1]);
            let row_of = |t: u32| order.iter().position(|&o| o == t).unwrap();
            let centre = |t: u32| v.row_top(row_of(t)) + v.row_height / 2.0;
            let (ys, yp) = (centre(1), centre(0));
            // The anchor is the bar's finish for FF and SF, its start otherwise.
            let anchor = if matches!(kind, LinkType::FF | LinkType::SF) {
                x1
            } else {
                x0
            };
            let mut reaches_anchor = false;
            for r in s.rects.iter().filter(|r| r.color == THEME.link) {
                let (rx0, rx1) = (f64::from(r.pos[0]), f64::from(r.pos[0] + r.size[0]));
                let (ry0, ry1) = (f64::from(r.pos[1]), f64::from(r.pos[1] + r.size[1]));
                // Nothing leaves the two rows (a few pixels of margin).
                let (lo, hi) = (ys.min(yp) - 4.0, ys.max(yp) + 4.0);
                assert!(
                    ry0 >= lo - 1.0 && ry1 <= hi + 1.0,
                    "{what}: {r:?} leaves the rows"
                );
                // On the successor's row, nothing is inside its bar.
                if ry0 < ys + 3.0 && ry1 > ys - 3.0 {
                    assert!(
                        rx1 <= x0 + 1.0 || rx0 >= x1 - 1.0,
                        "{what}: {r:?} is inside the bar"
                    );
                    reaches_anchor |= (rx0 - anchor).abs() <= 1.0 || (rx1 - anchor).abs() <= 1.0;
                }
            }
            assert!(
                reaches_anchor,
                "{what}: no arrow rect reaches the bar's end"
            );
        }
    }
}

#[test]
fn bars_draw_over_the_arrows() {
    let d = dates(&[]);
    let order: Vec<u32> = (0..N as u32).collect();
    let deps = vec![dep(502, 503), dep(506, 507), dep(510, 514)];
    let index = LinkIndex::new(&deps, |_| true, &order);
    let v = view();
    let mut s = scene::build(
        &v,
        1.0,
        &order,
        &d,
        &timeline::Summaries::default(),
        None,
        None,
        &THEME,
    );
    let (before, bars) = (s.rects.len(), s.bars);
    scene::add_arrows(&mut s, &v, 1.0, &d, &index, &THEME);
    assert_eq!((s.arrows, s.bars), (3, bars));
    let is_bar = |r: &scene::Rect| r.color == THEME.bar_soft || r.color == THEME.crit_soft;
    let last_arrow = s.rects.iter().rposition(|r| r.color == THEME.link).unwrap();
    let first_bar = s.rects.iter().position(is_bar).unwrap();
    assert!(
        last_arrow < first_bar,
        "arrow {last_arrow}, first bar {first_bar}"
    );
    assert!(s.rects.len() > before);
    assert_eq!(s.rects.iter().filter(|r| is_bar(r)).count(), bars);
}
