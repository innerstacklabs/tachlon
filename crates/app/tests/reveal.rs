//! The driving path lights up from the selected task back (gs#309), on a fake clock.

use app::edit::Edit;
use app::{Session, meta::TaskMeta, outline::Outline};
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use std::cell::Cell;
use timeline::Timeline;

thread_local! {
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.get()
}

fn at(ms: f64) {
    NOW.set(ms);
}

/// Seven tasks in a chain `0 -> 1 -> ... -> 6`, so task 5 has five tasks behind it and one ahead.
fn chain() -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7 calendar");
    let cal = schedule.add_calendar(cal);
    let ids: Vec<TaskId> = (0..7)
        .map(|_| {
            schedule
                .add_task(Task {
                    duration: 3 * 60,
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
    for pair in ids.windows(2) {
        schedule
            .add_dependency(Dependency {
                pred: pair[0],
                succ: pair[1],
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("valid dependency");
    }
    let solution = schedule.compute().expect("acyclic schedule");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    timeline.viewport.fit(0.0, solution.project_finish as f64);
    let meta: Vec<TaskMeta> = Vec::new();
    Session::new(schedule, solution, timeline, meta, clock)
}

/// The tasks the drawn highlight lights.
fn lit(session: &Session) -> Vec<u32> {
    let h = session.revealed_highlight().expect("a selection");
    (0..7).filter(|&t| h.has_task(t)).collect()
}

fn full(session: &Session) -> Vec<u32> {
    let h = session.selection().expect("a selection").highlight();
    (0..7).filter(|&t| h.has_task(t)).collect()
}

fn select(session: &mut Session, task: u32) {
    session.select(task);
    session.advance_reveal();
}

#[test]
fn the_path_lights_from_the_selected_task_back() {
    at(0.0);
    let mut s = chain();
    select(&mut s, 5);
    assert_eq!(s.selection().expect("selected").back.len(), 5);
    assert_eq!(lit(&s), vec![5], "only the selected task at t = 0");

    at(150.0);
    s.advance_reveal();
    let mid = lit(&s).len();
    assert!(mid > 1 && mid < full(&s).len(), "midpoint lit {mid}");
    let nearest_first = lit(&s);
    assert!(
        nearest_first.contains(&4),
        "the nearest driver lights first"
    );
    assert!(!nearest_first.contains(&6), "the forward chain waits");
    let h = s.revealed_highlight().expect("a selection");
    assert!(h.has_edge(4, 5), "a link between two lit tasks is lit");
    assert!(!h.has_edge(0, 1), "a link to an unlit task is not");
    assert!(!h.has_edge(5, 6), "the forward link waits with its task");

    at(300.0);
    s.advance_reveal();
    assert_eq!(lit(&s), full(&s));
    assert!(
        lit(&s).contains(&6),
        "the forward chain lights with the last frame"
    );
}

#[test]
fn reduced_motion_lights_the_whole_path_at_once() {
    at(0.0);
    let mut s = chain();
    select(&mut s, 5);
    assert_eq!(lit(&s), vec![5], "with motion, t = 0 lights one task");
    s.set_reduced_motion(true);
    select(&mut s, 4);
    assert_eq!(lit(&s), full(&s));
    assert!(full(&s).len() > 1);
}

#[test]
fn a_second_selection_restarts_the_reveal() {
    at(0.0);
    let mut s = chain();
    select(&mut s, 5);
    at(400.0);
    s.advance_reveal();
    assert_eq!(lit(&s), full(&s));
    select(&mut s, 4);
    assert_eq!(lit(&s), vec![4], "restarted at t = 0");
}

#[test]
fn an_edit_does_not_replay_it() {
    at(0.0);
    let mut s = chain();
    select(&mut s, 5);
    at(150.0);
    s.advance_reveal();
    let before = lit(&s);
    s.apply(Edit::SetDuration {
        task: TaskId(0),
        minutes: 6 * 60,
    })
    .expect("edit applies");
    s.refresh_selection();
    s.advance_reveal();
    assert_eq!(lit(&s), before, "the edit did not restart the reveal");
    at(400.0);
    s.advance_reveal();
    assert_eq!(lit(&s), full(&s));
    s.apply(Edit::SetDuration {
        task: TaskId(0),
        minutes: 3 * 60,
    })
    .expect("edit applies");
    s.refresh_selection();
    s.advance_reveal();
    assert_eq!(
        lit(&s),
        full(&s),
        "an edit after the reveal does not replay it"
    );
}

#[test]
fn selecting_the_same_task_again_does_not_restart_the_reveal() {
    at(0.0);
    let mut s = chain();
    select(&mut s, 5);
    at(150.0);
    s.advance_reveal();
    let before = lit(&s);
    assert!(before.len() > 1, "the reveal is under way");
    select(&mut s, 5);
    assert_eq!(lit(&s), before, "the same task kept its place");
}

/// Summary 0 over leaves 1, 2 and 3 (links 1 -> 2 and 2 -> 3), outside leaf 4.
fn summary() -> Session {
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
    for (pred, succ) in [(1, 2), (2, 3)] {
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
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("outline fits")
}

#[test]
fn a_summary_subtree_is_lit_at_t_zero() {
    at(0.0);
    let mut s = summary();
    select(&mut s, 0);
    let h = s.revealed_highlight().expect("a selection");
    assert!([0, 1, 2, 3].iter().all(|&t| h.has_task(t)));
    assert!(!h.has_task(4));
    assert!(h.has_edge(1, 2) && h.has_edge(2, 3));
}
