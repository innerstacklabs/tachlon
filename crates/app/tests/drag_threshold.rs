//! The bar-drag path of the app, without a canvas: the drag threshold, a
//! release or cancel with no frame, and a drag that returns to its press.

use crate::common;

use common::{TRAVEL_PX, setup, setup_with};

use app::Session;

/// Press on the task's bar, move `dx` px to the right (`None`: no move), and
/// release. The session, the task, and its SNET before the press.
fn press_move_release(dx: Option<f64>) -> (Session, u32, Option<i64>) {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    s.pointer_down(x, y, false);
    if let Some(dx) = dx {
        s.pointer_move(x + dx, y);
    }
    s.pointer_up();
    (s, task, before)
}

#[test]
fn a_click_with_no_move_edits_nothing() {
    let (s, task, before) = press_move_release(None);
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert!(s.drag_info().is_none());
}

#[test]
fn a_release_inside_the_threshold_edits_nothing() {
    let (s, task, before) = press_move_release(Some(2.0));
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert!(s.drag_info().is_none());
}

#[test]
fn the_threshold_is_euclidean() {
    // 2 px right and 2 px down is 2.83 px: still a click. 3 px right and
    // 1 px down is 3.16 px: a drag.
    let (mut s, _, _, (x, y)) = setup();
    s.pointer_down(x, y, false);
    s.pointer_move(x + 2.0, y + 2.0);
    s.pointer_up();
    assert_eq!(s.revision(), 0);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 3.0, y + 1.0);
    assert!(s.drag_started());
    s.pointer_up();
}

#[test]
fn a_release_past_the_threshold_commits_the_drag() {
    let (s, task, _) = press_move_release(Some(TRAVEL_PX));
    assert_eq!(s.revision(), 1);
    assert!(s.schedule().tasks()[task as usize].snet.is_some());
    assert!(s.drag_info().is_some());
}

#[test]
fn a_drag_that_started_stays_started_when_the_pointer_returns() {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    s.pointer_down(x, y, false);
    s.pointer_move(x + 40.0, y);
    s.pointer_move(x, y);
    assert!(s.drag_started(), "coming back does not end the drag");
    s.pointer_up();
    // Back at the press point the ask is the task's own start: nothing to commit.
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert!(!s.drag_info().expect("the frame recorded the ask").held);
}

#[test]
fn a_vertical_drag_edits_nothing() {
    // 5 px down is past the threshold, but the ask is the task's own start.
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x, y + 5.0);
    assert!(s.drag_started());
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert!(!s.drag_info().expect("the frame recorded the ask").held);
    assert_eq!(*s.solution(), start);
}

#[test]
fn a_drag_out_and_back_between_frames_edits_nothing() {
    let (mut s, task, _, (x, y)) = setup_with(true);
    let before = s.schedule().tasks()[task as usize].snet;
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x + TRAVEL_PX, y);
    s.pointer_move(x, y);
    s.pointer_up();
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert_eq!(*s.solution(), start);
}

#[test]
fn a_drag_out_and_back_across_frames_ends_where_it_began() {
    let (mut s, task, es, (x, y)) = setup_with(true);
    let before = s.schedule().tasks()[task as usize].snet;
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x + TRAVEL_PX, y);
    s.apply_drag();
    assert_eq!(s.revision(), 1);
    assert_eq!(s.stamp(), 1);
    assert_ne!(*s.solution(), start, "the frame moved the schedule");
    s.pointer_move(x, y);
    s.pointer_up();
    // The release puts the constraint back: the schedule is the one from the
    // press, so the revision is too. The table still has a new solution to
    // draw, which the stamp says.
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0, "an out-and-back drag leaves the revision");
    assert_eq!(s.stamp(), 2, "the table redraws the restored rows");
    assert_eq!(s.solution().dates[task as usize].es, es);
    assert_eq!(*s.solution(), start);
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

#[test]
fn a_cancel_after_a_frame_puts_the_task_back() {
    let (mut s, task, _, (x, y)) = setup_with(true);
    let before = s.schedule().tasks()[task as usize].snet;
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    s.pointer_move(x + TRAVEL_PX, y);
    s.apply_drag();
    assert_eq!(s.revision(), 1);
    assert_ne!(*s.solution(), start, "the frame moved the schedule");
    // More moves land after the frame, then the browser cancels the gesture.
    s.pointer_move(x + 2.0 * TRAVEL_PX, y);
    s.pointer_cancel();
    assert!(s.timeline().bar_drag().is_none(), "the drag ended");
    assert!(!s.drag_started());
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(*s.solution(), start);
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
    // The revert is the press-time schedule again, and a new solution for
    // the table to show.
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 2);
    assert!(s.drag_info().is_none());
    // The next frame has nothing left to apply.
    s.apply_drag();
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 2);
}
