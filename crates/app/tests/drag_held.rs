//! The bar-drag path of the app, without a canvas: a task a predecessor holds
//! past its constraint, the held outline, and the constraint put back on a
//! frame, release or cancel.

use crate::common;

use common::clock;

use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task, TaskId};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use timeline::Timeline;

use app::Session;

/// 08:00 on a day, in minutes.
const OPEN: i64 = 8 * 60;

/// Counts clock reads, so each timed recompute costs exactly 1.0.
static TICKS: AtomicU64 = AtomicU64::new(0);

fn ticking_clock() -> f64 {
    TICKS.fetch_add(1, SeqCst) as f64
}

/// A task whose constraint its predecessor overrides: `snet` is a day before
/// the early start the predecessor gives. One pixel is ten minutes, so a day
/// is 144 px. Returns the session, the task, its snet and early start, and
/// the canvas point to press on its bar.
fn held_task() -> (Session, u32, i64, i64, (f64, f64)) {
    held_task_with(true)
}

/// [`held_task`], with the task carrying its own snet only when `own_snet`:
/// without it the predecessor alone holds the task, and a drag adds the first
/// constraint it has.
fn held_task_with(own_snet: bool) -> (Session, u32, i64, i64, (f64, f64)) {
    held_task_with_clock(own_snet, clock)
}

/// [`held_task_with`], using `clock_fn` to time cone recomputes.
fn held_task_with_clock(
    own_snet: bool,
    clock_fn: fn() -> f64,
) -> (Session, u32, i64, i64, (f64, f64)) {
    let mut schedule = Schedule::new();
    let day = vec![(8 * 60, 17 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("valid calendar");
    let cal = schedule.add_calendar(cal);
    let snet = DAY + 8 * 60;
    let work = |snet| Task {
        duration: 9 * 60,
        calendar: cal,
        snet,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    let pred = schedule.add_task(work(Some(snet))).expect("valid task");
    let task = schedule
        .add_task(work(own_snet.then_some(snet)))
        .expect("valid task");
    schedule
        .add_dependency(Dependency {
            pred,
            succ: task,
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("valid dependency");
    let solution = schedule.compute().expect("acyclic");
    let es = solution.dates[task.0 as usize].es;
    assert!(es > snet, "the predecessor holds the task past its snet");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    let v = &mut timeline.viewport;
    v.px_per_minute = 0.1;
    v.t0 = es as f64 - 5000.0;
    let row = timeline
        .order()
        .iter()
        .position(|&t| t == task.0)
        .expect("the task has a row");
    let x = timeline.viewport.x_of(es as f64) + 2.0;
    let y = timeline.viewport.row_top(row) + timeline.viewport.row_height / 2.0;
    let session = Session::new(schedule, solution, timeline, Vec::new(), clock_fn);
    // The press must land on that bar, or the tests below check nothing.
    assert_eq!(
        session.timeline().hit_test(&session.solution().dates, x, y),
        Some(task.0)
    );
    (session, task.0, snet, es, (x, y))
}

/// A frame whose ask is the constraint the task already has recomputes
/// nothing, so it reports no recompute time of its own rather than the
/// previous frame's.
#[test]
fn a_frame_that_recomputes_nothing_reports_zero_recompute_time() {
    TICKS.store(0, SeqCst);
    let (mut s, task, snet, es, (x, y)) = held_task_with_clock(true, ticking_clock);
    s.pointer_down(x, y, false);

    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert!(
        s.drag_info()
            .expect("the first frame recorded the drag")
            .recompute_ms
            > 0.0
    );

    // Back to the first day, 08:00 Monday: the predecessor holds the task at
    // the press start, so the snet goes back to the press-time one.
    s.pointer_move(x - (es - OPEN) as f64 * 0.1, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(snet));
    assert!(
        s.drag_info()
            .expect("the second frame recorded the drag")
            .recompute_ms
            > 0.0
    );

    s.pointer_move(x - (es - snet) as f64 * 0.1, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(snet));
    assert_eq!(
        s.drag_info()
            .expect("the shortcut recorded the drag")
            .recompute_ms,
        0.0
    );
    s.pointer_cancel();
}

/// Deliberately gives `Session` a stale, corrupted solution so applying the
/// ask succeeds but restoring the press constraint fails. A consistent press
/// state cannot reach this branch; this pins that the successful ask remains.
#[test]
fn a_restore_that_fails_leaves_the_ask_applied() {
    let mut schedule = Schedule::new();
    let day = vec![(8 * 60, 17 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("valid calendar");
    let cal = schedule.add_calendar(cal);
    let moved = schedule
        .add_task(Task {
            duration: 8 * 60,
            calendar: cal,
            snet: Some(DAY + 8 * 60),
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("valid task");
    let other = schedule
        .add_task(Task {
            duration: 8 * 60,
            calendar: cal,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("valid task");
    let mut solution = schedule.compute().expect("independent tasks are feasible");
    schedule
        .set_snet(moved, Some(2 * DAY + 8 * 60))
        .expect("valid constraint");
    let moved_es = solution.dates[moved.0 as usize].es;
    // 300 minutes later is mid-day: the drop lands on the day's start.
    let ask = moved_es;
    solution.dates[other.0 as usize].es = 10 * DAY;

    let mut ask_schedule = schedule.clone();
    let mut ask_solution = solution.clone();
    ask_schedule.set_snet(moved, Some(ask)).expect("valid ask");
    assert!(
        ask_schedule
            .recompute_cone(&mut ask_solution, moved)
            .is_ok()
    );
    let mut restore_schedule = schedule.clone();
    let mut restore_solution = solution.clone();
    assert!(
        restore_schedule
            .recompute_cone(&mut restore_solution, moved)
            .is_err()
    );

    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    let row = timeline
        .order()
        .iter()
        .position(|&task| task == moved.0)
        .expect("the moved task has a row");
    timeline.viewport.px_per_minute = 0.1;
    timeline.viewport.t0 = moved_es as f64 - 5000.0;
    let x = timeline.viewport.x_of(moved_es as f64) + 2.0;
    let y = timeline.viewport.row_top(row) + timeline.viewport.row_height / 2.0;
    let mut s = Session::new(schedule, solution, timeline, Vec::new(), clock);
    assert_eq!(
        s.timeline().hit_test(&s.solution().dates, x, y),
        Some(moved.0)
    );

    s.pointer_down(x, y, false);
    s.pointer_move(x + 30.0, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[moved.0 as usize].snet, Some(ask));
    assert_eq!(s.revision(), 1);
    assert_eq!(s.stamp(), 1);
    let info = s.drag_info().expect("the successful ask was recorded");
    assert_eq!(info.wanted, ask);
    assert_eq!(info.es, ask);

    s.pointer_up();
    assert_eq!(s.schedule().tasks()[moved.0 as usize].snet, Some(ask));
    assert_eq!(s.revision(), 1);
    assert_eq!(s.stamp(), 1);
}

/// [`held_task`] pressed, dragged out a few days and applied, then dragged
/// to ask for a start at the task's own snet and applied. The second frame
/// asks for a start the predecessor holds at the press start, so the engine
/// start is the press start (the rule compares that, not the ask): the frame
/// puts back the snet the task had at the press, and the schedule, the
/// solution and the revision are the press-time ones. Returns the session,
/// the task, the press-time solution and the press point.
fn held_task_dragged_out_and_to_its_snet() -> (Session, u32, engine::Solution, (f64, f64)) {
    let (mut s, task, snet, es, (x, y)) = held_task();
    let start = s.solution().clone();
    assert_eq!(s.revision(), 0);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert_eq!(s.revision(), 1);
    assert_ne!(
        s.solution().dates[task as usize].es,
        es,
        "the frame moved it"
    );
    s.pointer_move(x - (es - snet) as f64 * 0.1, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(snet));
    assert_eq!(*s.solution(), start, "the held task is where it was");
    assert_eq!(
        s.revision(),
        0,
        "the frame restored the press-time schedule"
    );
    (s, task, start, (x, y))
}

/// [`held_task_with`] without its own snet, pressed and dragged 100 px left:
/// the ask is a day before the start the predecessor holds. Returns the
/// session, the task, the solution at the press and the press point.
fn predecessor_held_task_pulled_left() -> (Session, u32, engine::Solution, (f64, f64)) {
    let (mut s, task, snet, es, (x, y)) = held_task_with(false);
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x - 100.0, y);
    assert!(s.drag_started());
    // The ask is later than the snet a hidden constraint would be given
    // (the bug this guards), and earlier than the held start.
    // Both are day starts: a drop lands on the first working minute of a day.
    assert!(es - 1000 > snet);
    (s, task, start, (x, y))
}

/// The engine-computed start decides, not the snapped ask: a task held by a
/// predecessor keeps its start when dragged left, so no constraint is added.
/// The task is still held later than the pointer asked, and the page shows it.
#[test]
fn a_predecessor_held_task_pulled_left_adds_no_constraint() {
    let (mut s, task, start, _) = predecessor_held_task_pulled_left();
    let es = start.dates[task as usize].es;
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0, "nothing changed, so no redraw");
    assert!(s.held(), "the first frame shows the held outline");
    let info = s.drag_info().expect("the frame recorded the drag");
    assert!(info.held);
    assert_eq!(info.es, es);
    assert_eq!(*s.solution(), start);
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    assert!(s.drag_info().expect("still recorded").held);
    assert_eq!(*s.solution(), start);
}

#[test]
fn a_predecessor_held_task_pulled_left_and_released_adds_no_constraint() {
    // The release applies the ask itself, with no frame before it.
    let (mut s, task, start, _) = predecessor_held_task_pulled_left();
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    assert!(s.drag_info().expect("the release recorded the drag").held);
    assert_eq!(*s.solution(), start);
}

/// The spec's case: the task carries its own snet and a predecessor holds it
/// past that. Pulled left on the first frame, nothing changes, and the
/// task is shown held.
#[test]
fn a_held_task_with_its_own_snet_pulled_left_changes_nothing_and_shows_held() {
    let (mut s, task, snet, es, (x, y)) = held_task();
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x - 100.0, y);
    assert!(s.drag_started());
    // The ask is between the task's own snet and the start it is held at.
    assert!(es - 1000 > snet);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(snet));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0, "nothing changed, so no redraw");
    assert_eq!(*s.solution(), start);
    assert!(s.held());
    assert_eq!(s.drag_info().expect("the frame recorded the drag").es, es);
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(snet));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    assert_eq!(*s.solution(), start);
}

/// Pulled left it is held; back at the press point the ask is the start the
/// predecessor gives, so it is held no longer, on the frame and after the
/// release.
#[test]
fn a_predecessor_held_task_pulled_back_to_its_press_start_stops_showing_held() {
    let (mut s, task, start, (x, y)) = predecessor_held_task_pulled_left();
    let es = start.dates[task as usize].es;
    s.apply_drag();
    assert!(s.held());
    s.pointer_move(x, y);
    s.apply_drag();
    let info = s.drag_info().expect("the frame recorded the drag");
    assert_eq!(info.wanted, es);
    assert_eq!(info.es, es);
    assert!(!info.held, "the ask equals the early start");
    assert!(!s.held());
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    s.pointer_up();
    assert!(!s.drag_info().expect("still recorded").held);
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    assert_eq!(*s.solution(), start);
}

/// The same, with no frame at the press point.
#[test]
fn a_predecessor_held_task_pulled_back_and_released_stops_showing_held() {
    let (mut s, task, start, (x, y)) = predecessor_held_task_pulled_left();
    s.apply_drag();
    assert!(s.held());
    s.pointer_move(x, y);
    s.pointer_up();
    assert!(!s.drag_info().expect("the release recorded the drag").held);
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    assert_eq!(*s.solution(), start);
}

/// A press clears what the last drag recorded. Drags in both directions set
/// their own held result, so a stale held flag cannot carry over or stick.
#[test]
fn a_new_press_starts_with_the_held_outline_clear() {
    let (mut s, task, start, (x, y)) = predecessor_held_task_pulled_left();
    s.apply_drag();
    assert!(s.held());
    s.pointer_up();
    assert!(s.drag_info().is_some_and(|d| d.held));
    // The next press on the same task: nothing is held until a frame says so.
    s.pointer_down(x, y, false);
    assert!(s.drag_info().is_none());
    assert!(!s.held());
    // A drag out and applied is not held, and the earlier flag did not leak.
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert!(!s.held());
    s.pointer_cancel();
    assert_eq!(*s.solution(), start);
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    let stamp = s.stamp();
    // A third press checks the other direction and then returns to its start.
    s.pointer_down(x, y, false);
    assert!(s.drag_info().is_none());
    assert!(!s.held());
    s.pointer_move(x - 100.0, y);
    s.apply_drag();
    assert!(s.held());
    s.pointer_move(x, y);
    s.apply_drag();
    assert!(!s.held());
    s.pointer_cancel();
    assert_eq!(*s.solution(), start);
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.stamp(), stamp, "the third press changed no solution");
}

/// A drag that does move the start commits a constraint and a revision, and
/// pulling back to a start the predecessor holds takes both back.
#[test]
fn a_drag_that_moves_the_start_of_a_held_task_commits() {
    let (mut s, task, _, es, (x, y)) = held_task_with(false);
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    let wanted = s
        .schedule()
        .calendar_of(TaskId(task))
        .day_start_at(es + 3000);
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(wanted));
    assert_eq!(s.revision(), 1);
    assert_eq!(s.stamp(), 1);
    let info = s.drag_info().expect("the frame recorded the drag");
    assert!(!info.held);
    assert!(info.es >= wanted);
    assert_ne!(*s.solution(), start);
    // Back to the held start: the predecessor gives the press start again.
    s.pointer_move(x - 100.0, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[task as usize].snet, None);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 2, "the table redraws the restored rows");
    assert_eq!(*s.solution(), start);
    s.pointer_up();
    assert_eq!(s.revision(), 0);
    assert_eq!(*s.solution(), start);
}

#[test]
fn a_release_at_the_press_start_after_a_frame_that_set_the_snet_back_leaves_the_revision() {
    let (mut s, task, start, (x, y)) = held_task_dragged_out_and_to_its_snet();
    let snet = s.schedule().tasks()[task as usize].snet;
    let stamp = s.stamp();
    // The pointer returns to the press position and lets go. The snet is the
    // press-time one already, so the release has nothing to set; the
    // revision must still be the press-time one.
    s.pointer_move(x, y);
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, snet);
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), stamp, "no solution changed, so no redraw");
    assert_eq!(*s.solution(), start);
}

#[test]
fn a_release_right_after_a_frame_that_set_the_snet_back_leaves_the_revision() {
    // The release asks for what the last frame applied, so it skips the
    // recompute; the revision is still put right.
    let (mut s, task, start, _) = held_task_dragged_out_and_to_its_snet();
    s.pointer_up();
    assert_eq!(s.revision(), 0);
    assert_eq!(*s.solution(), start);
    assert!(s.schedule().tasks()[task as usize].snet.is_some());
}

#[test]
fn a_cancel_after_a_frame_that_set_the_snet_back_leaves_the_revision() {
    let (mut s, task, start, _) = held_task_dragged_out_and_to_its_snet();
    let snet = s.schedule().tasks()[task as usize].snet;
    s.pointer_cancel();
    assert!(!s.drag_started());
    assert_eq!(s.schedule().tasks()[task as usize].snet, snet);
    assert_eq!(s.revision(), 0);
    assert!(s.drag_info().is_none());
    assert_eq!(*s.solution(), start);
}
