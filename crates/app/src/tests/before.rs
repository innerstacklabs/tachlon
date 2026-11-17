//! A press that remembers a duration (`Before::Duration`): its release records one step that undo
//! puts back, and its cancel puts the duration back. The tests write the press and the drag's edit;
//! `tests/drag_resize.rs` presses the edge.

use super::*;
use crate::edit::Edit;

/// A session whose press on task 0 remembers its duration, after the drag lengthened the task by
/// a working day. Returns it with the duration at the press.
fn resized() -> (Session, i64) {
    let layout = outline::demo_layout(8);
    let timeline = Timeline::new(layout.order, 1000.0, 400.0, 1.0);
    let mut s = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        test_clock,
    );
    let at_press = s.schedule.tasks()[0].duration;
    s.press = Some(Press {
        at: (0.0, 0.0),
        task: 0,
        before: Before::Duration(at_press),
        es: s.solution().dates[0].es,
        revision: s.revision(),
    });
    s.drag_started = true;
    let minutes = at_press + 480;
    s.run(Edit::SetDuration {
        task: TaskId(0),
        minutes,
    })
    .expect("a leaf takes a duration");
    (s, at_press)
}

#[test]
fn a_resize_release_records_one_step_and_undo_puts_the_duration_back() {
    let (mut s, at_press) = resized();
    s.pointer_up();
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.schedule.tasks()[0].duration, at_press + 480);
    assert!(s.undo().expect("the step undoes"));
    assert_eq!(s.schedule.tasks()[0].duration, at_press);
    let full = s.schedule.clone().compute().expect("computes");
    assert_eq!(s.solution(), &full);
}

#[test]
fn a_cancelled_resize_puts_the_duration_back_and_records_nothing() {
    let (mut s, at_press) = resized();
    let revision = s.press.expect("the press is down").revision;
    s.pointer_cancel();
    assert_eq!(s.schedule.tasks()[0].duration, at_press);
    assert_eq!(s.history_len(), (0, 0));
    assert_eq!(s.revision(), revision);
    let full = s.schedule.clone().compute().expect("computes");
    assert_eq!(s.solution(), &full);
}
