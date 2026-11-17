//! Link ids, liveness, the link hit test, the drag preview and the selected
//! link (gs#344, part of gs#49).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{DAY, DepId, Dependency, LinkType, TaskDates, TaskId};
use std::time::Instant;
use timeline::scene::{
    self, LinkIndex, PreviewFrom, PreviewTo, Scene, add_link_preview, add_link_terminal,
    terminal_contains,
};
use timeline::{Theme, Timeline, Viewport};

const THEME: Theme = Theme::light();
/// Tasks 0 to 4 sit on rows 0 to 4; the canvas is 800 wide, one pixel per minute.
const N: usize = 5;

fn dates(spans: [(i64, i64); N]) -> Vec<TaskDates> {
    spans
        .iter()
        .map(|&(es, ef)| TaskDates {
            es,
            ef,
            ls: es,
            lf: ef,
            float: 0,
            finished: false,
        })
        .collect()
}

/// Task 0 ends at x 140; task 3 starts at 300 and task 4 at 303. The rest are
/// far right, out of every path.
fn fixture() -> (Timeline, Vec<TaskDates>) {
    let d = dates([(100, 140), (700, 740), (700, 740), (300, 340), (303, 343)]);
    let mut t = Timeline::new((0..N as u32).collect(), 800.0, 200.0, 1.0);
    t.viewport.t0 = 0.0;
    t.viewport.px_per_minute = 1.0;
    (t, d)
}

fn dep(pred: u32, succ: u32) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

fn order() -> Vec<u32> {
    (0..N as u32).collect()
}

/// 0 -> 3 (id 0) and 0 -> 4 (id 1). Link 0 runs: out (140..146, y 60), down
/// to y 72, across to x 294, down to y 132, in to the head at x 300.
fn two_links() -> LinkIndex {
    LinkIndex::new(&[dep(0, 3), dep(0, 4)], |_| true, &order())
}

#[test]
fn a_link_carries_its_dependency_id() {
    let (t, d) = fixture();
    let index = LinkIndex::new(&[dep(0, 3), dep(0, 4)], |_| true, &order());
    let ids: Vec<u32> = index
        .visible(&t.viewport, &d)
        .iter()
        .map(|p| p.link.dep)
        .collect();
    assert_eq!(ids, [0, 1]);
}

#[test]
fn a_removed_link_is_left_out_of_the_index() {
    let deps = [dep(0, 3), dep(0, 4), dep(1, 2)];
    let index = LinkIndex::new(&deps, |d| d != DepId(1), &order());
    assert_eq!(index.len(), 2);
    let (t, d) = fixture();
    let ids: Vec<u32> = index
        .visible(&t.viewport, &d)
        .iter()
        .map(|p| p.link.dep)
        .collect();
    assert_eq!(ids, [0, 2]);
}

#[test]
fn removed_links_neither_draw_nor_hit() {
    let (t, d) = fixture();
    let deps = [dep(0, 3), dep(0, 4)];
    let live = LinkIndex::new(&deps, |_| true, &order());
    let gone = LinkIndex::new(&deps, |_| false, &order());
    let draw = |index: &LinkIndex| {
        let mut s = Scene::default();
        scene::add_arrows_highlighted(&mut s, &t.viewport, 1.0, &d, index, None, None, &THEME);
        s.arrows
    };
    assert_eq!(draw(&live), 2);
    assert_eq!(draw(&gone), 0);
    assert_eq!(live.hit(&t.viewport, &d, 220.0, 72.0), Some(0));
    assert_eq!(gone.hit(&t.viewport, &d, 220.0, 72.0), None);
    let one = LinkIndex::new(&deps, |id| id == DepId(1), &order());
    assert_eq!(one.hit(&t.viewport, &d, 220.0, 72.0), Some(1));
}

#[test]
fn a_hit_on_every_segment_and_on_the_head() {
    let (t, d) = fixture();
    let index = LinkIndex::new(&[dep(0, 3)], |_| true, &order());
    let points = [
        (143.0, 60.0),  // out of the predecessor
        (146.0, 66.0),  // the half-row turn
        (220.0, 72.0),  // along the boundary between rows
        (294.0, 100.0), // down to the successor's row
        (297.0, 132.0), // into the successor
    ];
    for (x, y) in points {
        assert_eq!(index.hit(&t.viewport, &d, x, y), Some(0), "({x}, {y})");
    }
}

#[test]
fn a_point_five_pixels_away_misses() {
    let (t, d) = fixture();
    let index = LinkIndex::new(&[dep(0, 3)], |_| true, &order());
    let points = [
        (220.0, 77.0),  // below the boundary run
        (220.0, 67.0),  // above it
        (299.0, 100.0), // right of the vertical run
        (289.0, 100.0), // left of it
        (200.0, 100.0), // far from everything
    ];
    for (x, y) in points {
        assert_eq!(index.hit(&t.viewport, &d, x, y), None, "({x}, {y})");
    }
    assert_eq!(index.hit(&t.viewport, &d, 220.0, 76.0), Some(0));
}

#[test]
fn a_point_over_the_axis_band_misses() {
    let (t, d) = fixture();
    let index = LinkIndex::new(&[dep(0, 3)], |_| true, &order());
    assert_eq!(index.hit(&t.viewport, &d, 143.0, 40.0), None);
}

#[test]
fn a_bar_beats_a_link() {
    let (t, d) = fixture();
    let index = two_links();
    // Inside task 0's bar and two pixels from the stub leaving it.
    assert_eq!(index.hit(&t.viewport, &d, 139.0, 60.0), Some(0));
    assert_eq!(t.hit_test(&d, 139.0, 60.0), Some(0));
    assert_eq!(t.hit_link(&index, &d, 139.0, 60.0), None);
    // Past the bar's end the link is under the point.
    assert_eq!(t.hit_link(&index, &d, 143.0, 60.0), Some(0));
}

#[test]
fn the_nearest_link_wins() {
    let (t, d) = fixture();
    let index = two_links();
    // Link 0 turns down at x 294 and link 1 at x 297.
    assert_eq!(index.hit(&t.viewport, &d, 294.0, 100.0), Some(0));
    assert_eq!(index.hit(&t.viewport, &d, 295.0, 100.0), Some(0));
    assert_eq!(index.hit(&t.viewport, &d, 296.0, 100.0), Some(1));
    assert_eq!(index.hit(&t.viewport, &d, 297.0, 100.0), Some(1));
}

#[test]
fn a_link_along_a_row_boundary_is_hittable_under_a_bar_span() {
    // Task 1's bar (row 1, x 200 to 240) lies under link 0's boundary run at
    // y 72, which is the top edge of row 1 but above the drawn bar.
    let (t, mut d) = fixture();
    d[1] = TaskDates {
        es: 200,
        ef: 240,
        ls: 200,
        lf: 240,
        float: 0,
        finished: false,
    };
    let index = two_links();
    assert_eq!(t.hit_test(&d, 220.0, 72.0), Some(1), "row-wide bar test");
    assert_eq!(t.hit_link(&index, &d, 220.0, 72.0), Some(0));
    // On the drawn bar the bar still wins.
    assert_eq!(t.hit_link(&index, &d, 220.0, 84.0), None);
    assert_eq!(t.hit_link(&index, &d, 220.0, 76.0), Some(0));
}

#[test]
fn every_drawn_arrowhead_pixel_hits_the_link() {
    // For every link type and zoom, each corner and centre of the head (the last rectangle drawn
    // for a link, one triangle quad) hits the link.
    let n = 1;
    for zoom in [0.5, 1.0, 2.0] {
        let (mut t, d) = fixture();
        t.viewport.px_per_minute = zoom;
        for kind in [LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF] {
            let index = LinkIndex::new(&[Dependency { kind, ..dep(0, 3) }], |_| true, &order());
            let rects = arrow_rects(&index, &t, &d, None);
            assert!(rects.len() > n, "{kind:?} at {zoom}: a path and a head");
            // FF and SF enter task 3's finish and point left; FS and SS enter its start.
            let left = matches!(kind, LinkType::FF | LinkType::SF);
            let tip = t.viewport.x_of(if left { 340.0 } else { 300.0 });
            let (lo, hi) = if left {
                (tip, tip + scene::HEAD)
            } else {
                (tip - scene::HEAD, tip)
            };
            for r in &rects[rects.len() - n..] {
                let (x0, y0) = (f64::from(r.pos[0]), f64::from(r.pos[1]));
                let (x1, y1) = (x0 + f64::from(r.size[0]), y0 + f64::from(r.size[1]));
                assert!(lo <= x0 && x1 <= hi, "{kind:?} at {zoom}: head box at {x0}");
                let mid = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
                for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1), mid] {
                    let hit = index.hit(&t.viewport, &d, x, y);
                    assert_eq!(hit, Some(0), "{kind:?} at {zoom}: ({x}, {y})");
                }
            }
        }
    }
}

#[test]
fn a_point_only_the_head_covers_hits_the_link() {
    // Below the head's box by less than the slop, but further than the slop
    // from every run: only the head's own box can make this point hit.
    let (t, d) = fixture();
    let index = LinkIndex::new(&[dep(0, 3)], |_| true, &order());
    let rects = arrow_rects(&index, &t, &d, None);
    let tallest = rects[rects.len() - 1];
    let ty = f64::from(tallest.pos[1]) + f64::from(tallest.size[1]) / 2.0;
    let tip = t.viewport.x_of(300.0);
    let (x, y) = (
        tip - scene::HEAD / 2.0,
        ty + scene::HEAD + scene::HIT_SLOP - 0.5,
    );
    assert!(y - ty > scene::HIT_SLOP, "the last run is out of reach");
    const {
        assert!(
            scene::STUB >= scene::HEAD / 2.0,
            "the turn is behind the point"
        );
    };
    assert_eq!(index.hit(&t.viewport, &d, x, y), Some(0), "({x}, {y})");
}

#[test]
fn the_selected_link_is_drawn_after_every_other() {
    let (t, d) = fixture();
    let index = two_links();
    for pick in [0, 1] {
        let rects = arrow_rects(&index, &t, &d, Some(pick));
        let first_edge = rects
            .iter()
            .position(|r| r.color == THEME.sel_edge)
            .unwrap();
        let last_plain = rects.iter().rposition(|r| r.color == THEME.link).unwrap();
        assert!(last_plain < first_edge, "link {pick} paints last");
    }
}

#[test]
fn the_sizes_come_from_the_caller() {
    let (t, d) = fixture();
    let thick = |w: f64| {
        let mut s = Scene::default();
        let index = two_links();
        scene::add_arrows_highlighted(
            &mut s,
            &t.viewport,
            1.0,
            &d,
            &index,
            None,
            Some((0, w)),
            &THEME,
        );
        s.rects
            .iter()
            .any(|r| r.color == THEME.sel_edge && r.size[1] == w as f32 && r.size[0] > 100.0)
    };
    assert!(thick(2.0) && thick(3.0));
    let from = PreviewFrom { row: 0, x: 140.0 };
    let mut s = Scene::default();
    add_link_preview(
        &mut s,
        &t.viewport,
        1.0,
        from,
        PreviewTo::Bar(3, 300.0),
        false,
        3.0,
        &THEME,
    );
    assert!(
        s.rects
            .iter()
            .any(|r| r.size[1] == 3.0 && r.size[0] > 100.0)
    );
    let at = |gap: f64| {
        let mut s = Scene::default();
        add_link_terminal(&mut s, &t.viewport, 1.0, 0, &d[0], 8.0, gap, &THEME);
        s.rects[0].pos[0]
    };
    assert_eq!(at(7.0) - at(3.0), 4.0);
}

fn arrow_rects(
    index: &LinkIndex,
    t: &Timeline,
    d: &[TaskDates],
    selected: Option<u32>,
) -> Vec<scene::Rect> {
    let selected = selected.map(|dep| (dep, 2.0));
    let mut s = Scene::default();
    scene::add_arrows_highlighted(&mut s, &t.viewport, 1.0, d, index, None, selected, &THEME);
    s.rects
}

#[test]
fn the_selected_link_is_two_pixels_in_sel_edge() {
    let (t, d) = fixture();
    let index = two_links();
    let plain = arrow_rects(&index, &t, &d, None);
    let picked = arrow_rects(&index, &t, &d, Some(0));
    assert!(plain.iter().all(|r| r.color == THEME.link));
    let edge: Vec<_> = picked
        .iter()
        .filter(|r| r.color == THEME.sel_edge)
        .collect();
    let rest = picked.iter().filter(|r| r.color == THEME.link).count();
    assert_ne!(edge, [] as [&timeline::scene::Rect; 0]);
    assert_eq!(edge.len() + rest, picked.len());
    // Link 1 is untouched: its rectangles are the plain ones.
    assert_eq!(rest, plain.len() - edge.len());
    // The boundary run (y 72) is two pixels thick.
    assert!(edge.iter().any(|r| r.size[1] == 2.0 && r.size[0] > 100.0));
    assert!(plain.iter().any(|r| r.size[1] == 1.0 && r.size[0] > 100.0));
    // An id that names no visible link changes nothing.
    assert_eq!(arrow_rects(&index, &t, &d, Some(9)), plain);
}

#[test]
fn the_preview_turns_to_the_refused_colour() {
    let (t, _) = fixture();
    let from = PreviewFrom { row: 0, x: 140.0 };
    for to in [PreviewTo::Point(400.0, 150.0), PreviewTo::Bar(3, 300.0)] {
        let mut ok = Scene::default();
        let mut no = Scene::default();
        add_link_preview(&mut ok, &t.viewport, 1.0, from, to, false, 2.0, &THEME);
        add_link_preview(&mut no, &t.viewport, 1.0, from, to, true, 2.0, &THEME);
        assert_ne!(ok.rects, [] as [timeline::scene::Rect; 0]);
        assert!(ok.rects.iter().all(|r| r.color == THEME.sel_edge));
        assert!(no.rects.iter().all(|r| r.color == THEME.link_refused));
        assert_eq!(ok.rects.len(), no.rects.len());
    }
}

#[test]
fn the_preview_to_a_bar_follows_the_link_path() {
    let (t, d) = fixture();
    let mut s = Scene::default();
    add_link_preview(
        &mut s,
        &t.viewport,
        1.0,
        PreviewFrom { row: 0, x: 140.0 },
        PreviewTo::Bar(3, 300.0),
        false,
        2.0,
        &THEME,
    );
    let drawn = arrow_rects(
        &LinkIndex::new(&[dep(0, 3)], |_| true, &order()),
        &t,
        &d,
        Some(0),
    );
    let edge: Vec<_> = drawn.iter().filter(|r| r.color == THEME.sel_edge).collect();
    assert_eq!(s.rects.len(), edge.len());
    for (a, b) in s.rects.iter().zip(edge) {
        assert_eq!((a.pos, a.size), (b.pos, b.size));
    }
}

#[test]
fn the_preview_costs_the_same_at_1k_and_50k_tasks() {
    let count = |tasks: usize| {
        let mut v = Viewport::new(1000.0, 400.0, tasks);
        v.px_per_minute = 1.0;
        let mut s = Scene::default();
        let from = PreviewFrom { row: 3, x: 140.0 };
        let mut n = Vec::new();
        for to in [PreviewTo::Point(500.0, 200.0), PreviewTo::Bar(9, 300.0)] {
            add_link_preview(&mut s, &v, 1.0, from, to, false, 2.0, &THEME);
            n.push(s.rects.len());
        }
        n
    };
    assert_eq!(count(1_000), count(50_000));
}

#[test]
fn the_terminal_is_one_dot_outside_the_finish_end() {
    let (t, d) = fixture();
    let mut s = Scene::default();
    add_link_terminal(&mut s, &t.viewport, 1.0, 0, &d[0], 8.0, 3.0, &THEME);
    assert_eq!(s.rects.len(), 1);
    let r = s.rects[0];
    assert_eq!(r.color, THEME.link_terminal);
    // The bar ends at x 140; the dot starts to the right of that.
    assert!(r.pos[0] >= 140.0 && r.size == [8.0, 8.0]);
    assert_eq!(r.pos[1], 48.0 + 12.0 - 4.0);
    // A row above the canvas draws nothing.
    let mut none = Scene::default();
    add_link_terminal(&mut none, &t.viewport, 1.0, 40, &d[0], 8.0, 3.0, &THEME);
    assert_eq!(none.rects, [] as [timeline::scene::Rect; 0]);
}

#[test]
fn a_press_lands_on_the_terminal_where_it_is_drawn() {
    let (t, d) = fixture();
    let mut s = Scene::default();
    add_link_terminal(&mut s, &t.viewport, 1.0, 0, &d[0], 8.0, 3.0, &THEME);
    let (left, top) = (f64::from(s.rects[0].pos[0]), f64::from(s.rects[0].pos[1]));
    let centre = (left + 4.0, top + 4.0);
    let on = |at: (f64, f64)| terminal_contains(&t.viewport, 0, &d[0], 8.0, 3.0, at);
    assert!(on(centre));
    // The slop reaches 2 px past the dot on every side, and no further.
    assert!(on((left - 2.0, centre.1)) && !on((left - 2.5, centre.1)));
    assert!(on((left + 10.0, centre.1)) && !on((left + 10.5, centre.1)));
    assert!(on((centre.0, top - 2.0)) && !on((centre.0, top - 2.5)));
    // The bar itself is not the terminal, nor is another row's end.
    assert!(!on((120.0, centre.1)));
    assert!(!terminal_contains(&t.viewport, 1, &d[1], 8.0, 3.0, centre));
}

/// `cargo test -p timeline --test link_hit -- --ignored --nocapture`
#[test]
#[ignore = "timed"]
fn visible_at_50k_tasks() {
    const TASKS: usize = 50_000;
    let d: Vec<TaskDates> = (0..TASKS as i64)
        .map(|i| TaskDates {
            es: i * DAY,
            ef: i * DAY + DAY / 2,
            ls: i * DAY,
            lf: i * DAY + DAY / 2,
            float: 0,
            finished: false,
        })
        .collect();
    let deps: Vec<Dependency> = (0..TASKS as u32 - 1).map(|i| dep(i, i + 1)).collect();
    let order: Vec<u32> = (0..TASKS as u32).collect();
    let index = LinkIndex::new(&deps, |_| true, &order);
    let mut v = Viewport::new(1200.0, 800.0, TASKS);
    v.px_per_minute = 1200.0 / (40.0 * DAY as f64);
    v.t0 = 25_000.0 * DAY as f64;
    v.scroll_to_row(25_000);
    let runs = 200;
    let start = Instant::now();
    let mut seen = 0;
    for _ in 0..runs {
        seen = index.visible(&v, &d).len();
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
    println!("visible() at {TASKS} tasks: {per:.3} ms per call, {seen} links");
    assert!(seen > 0);
}
