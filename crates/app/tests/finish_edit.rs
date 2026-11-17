//! Typing the Finish cell (gs#703): the start stays and the duration becomes the working time from
//! it to the end of the typed day, as one undo step. The chain of `start_edit`: three tasks of eight
//! working hours, Monday to Friday, day 0 is Monday 2026-01-05.

use crate::start_edit::session;
use app::edit::{Edit, EditError, finish::FinishError};
use engine::TaskId;

const DAY_MINUTES: i64 = 8 * 60;

#[test]
fn a_finish_three_working_days_after_the_start_makes_a_three_day_duration_and_undo_restores_it() {
    let mut s = session();
    let start = s.solution().dates[0].es;
    s.set_finish_text(TaskId(0), "2026-01-07").unwrap();
    assert_eq!(s.schedule().tasks()[0].duration, 3 * DAY_MINUTES);
    assert_eq!(s.solution().dates[0].es, start, "the start stays");
    assert_eq!(s.date_text(s.solution().dates[0].ef), "2026-01-07 17:00");
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
    assert!(s.undo().unwrap());
    assert_eq!(s.schedule().tasks()[0].duration, DAY_MINUTES);
    // A Saturday ends the task on the Friday.
    s.set_finish_text(TaskId(0), "2026-01-10").unwrap();
    assert_eq!(s.date_text(s.solution().dates[0].ef), "2026-01-09 17:00");
    assert!(s.undo().unwrap());
    assert!(!s.undo().unwrap(), "one history entry per edit");
}

#[test]
fn a_refused_finish_says_why_and_changes_nothing() {
    let mut s = session();
    let stamp = s.stamp();
    let error = s.set_finish_text(TaskId(1), "2026-01-05").unwrap_err();
    assert_eq!(
        error.to_string(),
        "That day is before the task starts on 2026-01-06. Write a day on or after it."
    );
    assert_eq!(
        s.set_finish_text(TaskId(0), "soon"),
        Err(FinishError::Edit(EditError::BadDate))
    );
    s.set_finish_text(TaskId(0), "2026-01-05").unwrap();
    assert_eq!(s.stamp(), stamp, "nothing changed");
    assert!(!s.undo().unwrap(), "no history entry");
}

#[test]
fn a_summary_refuses_a_typed_finish() {
    let s = &mut crate::common::collapse::session(30);
    let summary = crate::common::collapse::summaries(s)[0];
    let error = s.set_finish_text(summary, "2026-01-12").unwrap_err();
    assert!(matches!(error, FinishError::Summary { .. }), "{error}");
}

#[test]
fn a_task_with_actuals_refuses_a_typed_finish_and_changes_nothing() {
    let mut s = session();
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(8 * 60),
        actual_finish: None,
        remaining: Some(8 * 60),
    })
    .unwrap();
    let stamp = s.stamp();
    let error = s.set_finish_text(TaskId(0), "2026-01-07").unwrap_err();
    assert_eq!(error, FinishError::Edit(EditError::HasActuals));
    assert!(error.to_string().contains("actual start or finish"));
    assert_eq!(s.schedule().tasks()[0].duration, DAY_MINUTES);
    assert_eq!(s.stamp(), stamp, "nothing changed");
}
