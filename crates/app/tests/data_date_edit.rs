//! Moving the data date (gs#703, story 5): the schedule recomputes, undo puts back the data date
//! and every date, redo repeats it. The dates are worked out by hand: one task of eight working
//! hours on a Monday-to-Friday calendar with 08:00-12:00 and 13:00-17:00 shifts, no predecessor;
//! day 0 is the origin Monday, so Monday 08:00 is minute 480.

use crate::common;

use app::{
    Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;

fn monday(hour: i64) -> Instant {
    hour * HOUR
}

fn wednesday(hour: i64) -> Instant {
    2 * DAY + hour * HOUR
}

/// One task of eight working hours with no predecessor, and the session over it. `shown` is the
/// data date the session marks before any edit.
fn lone_task(shown: Option<Instant>) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    schedule
        .add_task(Task {
            duration: 8 * HOUR,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("the calendar belongs to the schedule");
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new(vec![0], 1_000.0, 400.0, 1.0);
    Session::new(
        schedule,
        solution,
        timeline,
        vec![TaskMeta::default()],
        clock,
    )
    .with_data_date(shown)
}

fn dates(session: &Session) -> (Instant, Instant) {
    let d = session.solution().get(TaskId(0));
    (d.es, d.ef)
}

fn set(session: &mut Session, date: Instant) {
    session
        .edit(Edit::SetDataDate { date: Some(date) })
        .expect("the data date moves");
}

#[test]
fn moving_the_data_date_recomputes_and_undo_restores_it() {
    let mut session = lone_task(None);
    // The data date on Monday 08:00 changes nothing: the task runs Monday 08:00 to 17:00.
    set(&mut session, monday(8));
    assert_eq!(dates(&session), (monday(8), monday(17)));
    assert_eq!(session.data_date(), Some(monday(8)));
    assert_eq!(session.schedule().data_date(), Some(monday(8)));

    // Wednesday 00:00: the unstarted task starts at the next working instant, Wednesday 08:00, and
    // its eight hours end Wednesday 17:00, the project finish.
    let stamp = session.stamp();
    set(&mut session, wednesday(0));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));
    assert_eq!(session.solution().project_finish, wednesday(17));
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(session.schedule().data_date(), Some(wednesday(0)));
    assert_ne!(session.stamp(), stamp, "the table and toolbar redraw");

    // Undo puts back every date and the data date; redo repeats the move.
    assert!(session.undo().expect("undoes"));
    assert_eq!(dates(&session), (monday(8), monday(17)));
    assert_eq!(session.solution().project_finish, monday(17));
    assert_eq!(session.data_date(), Some(monday(8)));
    assert_eq!(session.schedule().data_date(), Some(monday(8)));
    assert!(session.redo().expect("redoes"));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(session.schedule().data_date(), Some(wednesday(0)));
}

/// A file opened today marks its data date on the session while the schedule holds none (the
/// loader does not set the schedule's yet, gs#736). Undo puts back that state exactly, not one
/// where the two agree.
#[test]
fn undo_restores_a_data_date_the_schedule_did_not_hold() {
    let mut session = lone_task(Some(monday(8)));
    assert_eq!(session.schedule().data_date(), None);

    set(&mut session, wednesday(0));
    assert_eq!(session.schedule().data_date(), Some(wednesday(0)));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));

    assert!(session.undo().expect("undoes"));
    assert_eq!(
        session.data_date(),
        Some(monday(8)),
        "the line is where it was"
    );
    assert_eq!(
        session.schedule().data_date(),
        None,
        "the floor is as it was"
    );
    assert_eq!(dates(&session), (monday(8), monday(17)));

    // Redo, then undo again, lands in the same place.
    assert!(session.redo().expect("redoes"));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));
    assert!(session.undo().expect("undoes"));
    assert_eq!(session.schedule().data_date(), None);
}

#[test]
fn clearing_the_data_date_is_one_undo_step() {
    let mut session = lone_task(None);
    set(&mut session, wednesday(0));
    session
        .edit(Edit::SetDataDate { date: None })
        .expect("clears");
    assert_eq!(session.data_date(), None);
    assert_eq!(dates(&session), (monday(8), monday(17)));
    assert!(session.undo().expect("undoes"));
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));
}

/// `apply` records nothing and its inverse is one edit, which cannot hold both a floor and a
/// line that differ; a data-date edit goes through `edit`, whose history keeps both. `apply`
/// refuses it and changes nothing, so no applied edit loses the floor.
#[test]
fn apply_refuses_a_data_date_edit_and_changes_nothing() {
    let mut session = lone_task(Some(monday(8)));
    assert_eq!(session.schedule().data_date(), None);
    let stamp = session.stamp();
    let error = session
        .apply(Edit::SetDataDate {
            date: Some(wednesday(0)),
        })
        .unwrap_err();
    assert_eq!(error, EditError::DataDateThroughEdit);
    assert_eq!(session.data_date(), Some(monday(8)));
    assert_eq!(
        session.schedule().data_date(),
        None,
        "the floor is as it was"
    );
    assert_eq!(dates(&session), (monday(8), monday(17)));
    assert_eq!(session.stamp(), stamp);
    assert_eq!(session.history_len(), (0, 0));
}

/// The text path the box takes: a day sets the date, empty text clears it, both one undo step.
#[test]
fn empty_text_clears_the_data_date_and_undo_restores_it() {
    let mut session = lone_task(None);
    session.set_data_date_text("2026-01-07").expect("sets");
    assert_eq!(session.schedule().data_date(), Some(wednesday(0)));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));

    session.set_data_date_text("  ").expect("clears");
    assert_eq!(session.data_date(), None);
    assert_eq!(session.schedule().data_date(), None, "the floor is gone");
    assert_eq!(dates(&session), (monday(8), monday(17)));
    assert_eq!(session.history_len(), (2, 0), "set and clear: two steps");

    assert!(session.undo().expect("undoes the clear"));
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(session.schedule().data_date(), Some(wednesday(0)));
    assert_eq!(dates(&session), (wednesday(8), wednesday(17)));

    assert!(session.redo().expect("redoes the clear"));
    assert_eq!(session.data_date(), None);
    assert_eq!(session.schedule().data_date(), None);
    assert_eq!(dates(&session), (monday(8), monday(17)));
}

/// Clearing a file's data date (the line, with no floor yet, gs#736) removes the line and undo
/// puts the line back with the floor still none.
#[test]
fn clearing_a_data_date_the_schedule_did_not_hold_undoes_exactly() {
    let mut session = lone_task(Some(monday(8)));
    session.set_data_date_text("").expect("clears");
    assert_eq!(session.data_date(), None);
    assert!(session.undo().expect("undoes"));
    assert_eq!(session.data_date(), Some(monday(8)));
    assert_eq!(session.schedule().data_date(), None);
}

#[test]
fn setting_the_data_date_already_in_force_is_not_an_edit() {
    let mut session = lone_task(None);
    session.set_data_date_text("2026-01-07").expect("sets");
    let (stamp, held) = (session.stamp(), dates(&session));
    session
        .set_data_date_text("2026-01-07")
        .expect("the same day again");
    assert_eq!(session.history_len(), (1, 0), "no second step");
    assert_eq!(session.stamp(), stamp, "nothing recomputed");
    assert_eq!(dates(&session), held);
    // A data date at 08:00 is not 00:00 on its day: setting the day moves it, as one step.
    let mut session = lone_task(Some(wednesday(8)));
    session
        .set_data_date_text("2026-01-07")
        .expect("moves to 00:00");
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(session.history_len(), (1, 0));
}

#[test]
fn clearing_with_no_data_date_is_not_an_edit() {
    let mut session = lone_task(None);
    let stamp = session.stamp();
    session.set_data_date_text("").expect("nothing to clear");
    assert_eq!(session.history_len(), (0, 0));
    assert_eq!(session.stamp(), stamp);
}

#[test]
fn text_that_is_not_a_day_is_refused_and_changes_nothing() {
    let mut session = lone_task(None);
    session.set_data_date_text("2026-01-07").expect("sets");
    for bad in ["2026-02-30", "next week", "2026-1-7"] {
        let error = session.set_data_date_text(bad).unwrap_err();
        assert_eq!(error, EditError::BadDataDate, "{bad}");
        assert_eq!(error.to_string(), "Write the data date as YYYY-MM-DD.");
    }
    assert_eq!(session.data_date(), Some(wednesday(0)));
    assert_eq!(session.history_len(), (1, 0));
}

#[test]
fn a_busy_session_refuses_and_changes_nothing() {
    let (mut session, _, _, point) = common::setup();
    session.pointer_down(point.0, point.1, false);
    let before = (session.data_date(), session.schedule().data_date());
    let stamp = session.stamp();
    let error = session
        .edit(Edit::SetDataDate {
            date: Some(wednesday(0)),
        })
        .unwrap_err();
    assert_eq!(error, EditError::Busy);
    assert_eq!(
        (session.data_date(), session.schedule().data_date()),
        before
    );
    assert_eq!(session.stamp(), stamp);
    session.pointer_up();
}
