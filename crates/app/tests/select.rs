//! Click selection and driving-path highlighting through the app's native
//! `Session`, without a canvas or browser.

use crate::common;

use app::{CHAIN_CAP, Session, meta::TaskMeta, outline::Outline};
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use std::cell::Cell;
use timeline::Timeline;
use timeline::scene::DIM;

thread_local! {
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.get()
}

fn set_clock(ms: f64) {
    NOW.set(ms);
}

/// A 24/7 schedule whose hand-known driving path is:
/// A -> B -> D -> E, with the non-driving alternatives A -> C -> D.
fn setup() -> (Session, [u32; 5]) {
    setup_with_clock(clock)
}

#[expect(clippy::many_single_char_names, reason = "the tasks are A to E")]
fn setup_with_clock(clock_fn: fn() -> f64) -> (Session, [u32; 5]) {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let cal = schedule.add_calendar(cal);
    let mut add = |hours: i64| {
        schedule
            .add_task(Task {
                duration: hours * 60,
                calendar: cal,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("valid task")
    };
    let a = add(3);
    let b = add(4);
    let c = add(2);
    let d = add(5);
    let e = add(1);
    for (pred, succ) in [(a, b), (a, c), (b, d), (c, d), (d, e)] {
        schedule
            .add_dependency(Dependency {
                pred,
                succ,
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("valid dependency");
    }
    let solution = schedule.compute().expect("acyclic schedule");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    timeline.viewport.fit(0.0, solution.project_finish as f64);
    (
        Session::new(schedule, solution, timeline, Vec::new(), clock_fn),
        [a.0, b.0, c.0, d.0, e.0],
    )
}

fn bar_point(session: &Session, task: u32) -> (f64, f64) {
    let row = session
        .timeline()
        .order()
        .iter()
        .position(|&t| t == task)
        .expect("task has a row");
    let dates = &session.solution().dates[task as usize];
    let x = session
        .timeline()
        .viewport
        .x_of((dates.es + dates.ef) as f64 / 2.0);
    let y = session.timeline().viewport.row_top(row) + session.timeline().viewport.row_height / 2.0;
    assert_eq!(
        session.timeline().hit_test(&session.solution().dates, x, y),
        Some(task)
    );
    (x, y)
}

fn click(session: &mut Session, task: u32) {
    let (x, y) = bar_point(session, task);
    session.pointer_down(x, y, false);
    session.pointer_up();
}

#[test]
fn a_click_on_a_bar_selects_it() {
    set_clock(0.0);
    let (mut s, [_, b, c, _, _]) = setup();
    click(&mut s, b);

    assert_eq!(s.selection().expect("selected").task, b);
    assert!(s.highlight().expect("highlight").has_task(b));
    let scene = s.scene();
    let theme = *s.theme();
    for &(task, index) in &scene.bar_rects {
        let color = scene.rects[index].color;
        if task == b {
            assert_eq!(color, theme.sel_edge);
        } else if task == c {
            let original = if s.solution().dates[task as usize].is_critical() {
                theme.crit_soft
            } else {
                theme.bar_soft
            };
            assert_eq!(color[..3], original[..3]);
            assert_eq!(color[3], original[3] * DIM);
        }
    }
    assert_eq!(s.revision(), 0);
}

#[test]
fn a_five_pixel_move_does_not_select() {
    set_clock(0.0);
    let (mut s, [_, b, _, _, _]) = setup();
    let (x, y) = bar_point(&s, b);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 5.0, y);
    s.pointer_up();

    // Five pixels is a drag, not a click, whether or not it crosses a day.
    assert!(s.selection().is_none());
}

#[test]
#[expect(clippy::many_single_char_names, reason = "the tasks are A to E")]
fn a_slow_press_does_not_select() {
    set_clock(0.0);
    let (mut s, [a, b, _, _, _]) = setup();
    let (x, y) = bar_point(&s, b);
    s.pointer_down(x, y, false);
    set_clock(301.0);
    s.pointer_up();
    assert!(s.selection().is_none());

    set_clock(1_000.0);
    let (x, y) = bar_point(&s, a);
    s.pointer_down(x, y, false);
    set_clock(1_299.0);
    s.pointer_up();
    assert_eq!(s.selection().expect("299 ms is a click").task, a);
}

#[test]
fn a_click_on_empty_space_clears_the_selection() {
    set_clock(0.0);
    let (mut s, [a, b, _, _, _]) = setup();
    click(&mut s, a);
    assert!(s.selection().is_some());

    s.pointer_down(500.0, 5.0, false);
    s.pointer_move(510.0, 5.0);
    s.pointer_up();
    assert_eq!(s.selection().expect("a pan does not clear").task, a);

    s.pointer_down(500.0, 5.0, false);
    s.pointer_up();
    assert!(s.selection().is_none());

    click(&mut s, b);
    s.clear_selection();
    assert!(s.selection().is_none());
}

#[test]
#[expect(clippy::many_single_char_names, reason = "the tasks are A to E")]
fn the_chain_follows_driving_links_only() {
    set_clock(0.0);
    let (mut s, [a, b, c, d, e]) = setup();
    click(&mut s, e);
    let selected = s.selection().expect("E selected");
    assert_eq!(selected.back, [d, b, a]);
    assert!(!selected.back_capped);
    assert_eq!(selected.forward, [] as [u32; 0]);
    assert!(!selected.highlight().has_task(c));
    for edge in [(d, e), (b, d), (a, b)] {
        assert!(selected.highlight().has_edge(edge.0, edge.1));
    }
    for edge in [(c, d), (a, c)] {
        assert!(!selected.highlight().has_edge(edge.0, edge.1));
    }

    click(&mut s, a);
    let selected = s.selection().expect("A selected");
    assert_eq!(selected.forward, [b, c, d, e]);
    assert!(!selected.forward_capped);
}

#[test]
fn the_chain_is_cached_until_a_recompute() {
    set_clock(0.0);
    let (mut s, [_, b, _, _, _]) = setup();
    s.select(b);
    assert_eq!(s.chain_builds(), 1);
    for _ in 0..3 {
        let _ = s.scene();
        s.refresh_selection();
    }
    assert_eq!(s.chain_builds(), 1);

    let (x, y) = bar_point(&s, b);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 5.0, y);
    s.pointer_up();
    assert_eq!(s.chain_builds(), 1);
    s.refresh_selection();
    assert_eq!(s.chain_builds(), 2);

    let (mut unchanged, [_, b, _, _, _]) = setup();
    unchanged.select(b);
    let (x, y) = bar_point(&unchanged, b);
    unchanged.pointer_down(x, y, false);
    unchanged.pointer_move(x, y + 10.0);
    unchanged.pointer_cancel();
    unchanged.refresh_selection();
    assert_eq!(unchanged.chain_builds(), 1);

    unchanged.pointer_down(x, y, false);
    unchanged.pointer_move(x, y + 10.0);
    unchanged.apply_drag();
    let before_cancel = unchanged.chain_builds();
    unchanged.pointer_cancel();
    unchanged.refresh_selection();
    assert!(unchanged.chain_builds() <= before_cancel + 1);
}

#[test]
fn the_chain_is_capped_at_two_thousand() {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let cal = schedule.add_calendar(cal);
    let tasks: Vec<TaskId> = (0..2_600)
        .map(|_| {
            schedule
                .add_task(Task {
                    duration: 60,
                    calendar: cal,
                    snet: None,
                    limit: engine::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                })
                .expect("valid task")
        })
        .collect();
    for pair in tasks.windows(2) {
        schedule
            .add_dependency(Dependency {
                pred: pair[0],
                succ: pair[1],
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("valid dependency");
    }
    let solution = schedule.compute().expect("chain is acyclic");
    let timeline = Timeline::new(
        timeline::order_by_start(&solution.dates),
        1000.0,
        400.0,
        1.0,
    );
    let mut s = Session::new(schedule, solution, timeline, Vec::new(), clock);

    s.select(2_599);
    let selected = s.selection().expect("last selected");
    assert_eq!(selected.back.len(), CHAIN_CAP);
    assert!(selected.back_capped);
    assert_eq!(selected.forward, [] as [u32; 0]);
    assert!(!selected.forward_capped);

    s.select(0);
    let selected = s.selection().expect("first selected");
    assert_eq!(selected.forward.len(), CHAIN_CAP);
    assert!(selected.forward_capped);
}

/// Summary S (0) over leaves a (1), b (2) and c (3); leaf d (4) is outside S.
/// Links: a -> b and b -> c inside S, c -> d from a descendant to outside.
fn summary_session(with_outline: bool) -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let cal = schedule.add_calendar(cal);
    let ids: Vec<TaskId> = (0..5)
        .map(|_| {
            schedule
                .add_task(Task {
                    duration: 60,
                    calendar: cal,
                    snet: None,
                    limit: engine::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                })
                .expect("valid task")
        })
        .collect();
    for (pred, succ) in [(1, 2), (2, 3), (3, 4)] {
        schedule
            .add_dependency(Dependency {
                pred: ids[pred],
                succ: ids[succ],
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("valid dependency");
    }
    let solution = schedule.compute().expect("acyclic schedule");
    let meta: Vec<TaskMeta> = (0..5)
        .map(|i| {
            let child = (1..=3).contains(&i);
            TaskMeta {
                name: format!("task {i}"),
                parent: child.then_some(TaskId(0)),
                outline_level: if child { 2 } else { 1 },
                code: None,
                ..TaskMeta::default()
            }
        })
        .collect();
    let wbs: Vec<TaskId> = (0..5).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid outline");
    let order = timeline::order_by_start(&solution.dates);
    let timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock);
    if with_outline {
        session.with_outline(outline).expect("outline fits")
    } else {
        session
    }
}

#[test]
fn a_summary_selection_lights_the_links_between_its_descendants() {
    let mut s = summary_session(true);
    s.select(0);
    let highlight = s.selection().expect("S selected").highlight();
    assert!(highlight.has_edge(1, 2));
    assert!(highlight.has_edge(2, 3));
    assert!(
        !highlight.has_edge(3, 4),
        "a link leaving the subtree stays dim"
    );
    assert!(!highlight.has_edge(2, 1));
}

#[test]
fn a_leaf_selection_highlight_is_unchanged_by_the_outline() {
    let mut outlined = summary_session(true);
    let mut flat = summary_session(false);
    for task in 1..5 {
        outlined.select(task);
        flat.select(task);
        let (with, without) = (
            outlined.selection().expect("selected").highlight(),
            flat.selection().expect("selected").highlight(),
        );
        for a in 0..5 {
            assert_eq!(with.has_task(a), without.has_task(a));
            for b in 0..5 {
                assert_eq!(with.has_edge(a, b), without.has_edge(a, b), "{a}->{b}");
            }
        }
    }
}

/// Measured: `cargo test --release -p app --test it -- --nocapture select::a_root_summary`.
/// The 50,000-task stress schedule (dense cross-phase links) under one summary.
#[test]
fn a_root_summary_selection_on_the_50k_fixture_is_fast() {
    let (mut s, root) = common::rooted_stress_session(50_000);
    let links = s.schedule().dependencies().len();
    let size = s.outline().expect("outline").subtree(root).len();
    assert_eq!(size, 50_001, "the root summary covers every task");
    assert_eq!(size, s.schedule().tasks().len());
    let start = std::time::Instant::now();
    s.select(root.0);
    let elapsed = start.elapsed();
    println!("root_summary select: {elapsed:?} ({size} tasks in subtree, {links} links)");
    let highlight = s.selection().expect("selected").highlight();
    for d in s.schedule().dependencies() {
        assert!(highlight.has_edge(d.pred.0, d.succ.0));
    }
    assert!(links > 0);
}
