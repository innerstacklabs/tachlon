//! A typed field planned as an edit (gantt#1875): each `plan_*_text` returns the edit its setter
//! would run, and running that edit through `edit_group` leaves the same dates and the same history
//! as the setter. The chain of `start_edit`: three tasks of eight working hours, day 0 is Monday
//! 2026-01-05.

use crate::start_edit::session;
use app::edit::EditError;
use app::history::Source;
use app::{Session, edit::Edit};
use engine::TaskId;

const FIRST: TaskId = TaskId(0);

/// Run the setter on one session and the plan's edit through `edit_group` on another: the
/// solutions and the history lengths must match, and the planned edit must change something.
fn same(plan: impl Fn(&Session) -> Edit, setter: impl Fn(&mut Session)) {
    let (mut by_setter, mut by_group) = (session(), session());
    let untouched = by_group.solution().clone();
    setter(&mut by_setter);
    let edit = plan(&by_group);
    by_group
        .edit_group(vec![edit], Source::Typed)
        .expect("the planned edit runs");
    assert_ne!(*by_group.solution(), untouched, "the plan moved a date");
    assert_eq!(by_group.solution(), by_setter.solution());
    assert_eq!(by_group.history_len(), by_setter.history_len());
    assert_eq!(by_group.history_len(), (1, 0), "one undo step");
}

#[test]
fn a_planned_start_runs_as_the_setter_does() {
    same(
        |s| s.plan_start_text(FIRST, "2026-01-12").unwrap().1.unwrap(),
        |s| {
            let _ = s.set_start_text(FIRST, "2026-01-12").unwrap();
        },
    );
}

#[test]
fn a_planned_finish_runs_as_the_setter_does() {
    same(
        |s| s.plan_finish_text(FIRST, "2026-01-07").unwrap().unwrap(),
        |s| s.set_finish_text(FIRST, "2026-01-07").unwrap(),
    );
}

#[test]
fn a_planned_percent_runs_as_the_setter_does() {
    same(
        |s| s.plan_percent_text(FIRST, "50%").unwrap(),
        |s| s.set_percent_text(FIRST, "50%").unwrap(),
    );
}

#[test]
fn a_planned_progress_runs_as_the_setter_does() {
    same(
        |s| s.plan_progress_text(FIRST, "2026-01-06", "", "4h").unwrap(),
        |s| s.set_progress_text(FIRST, "2026-01-06", "", "4h").unwrap(),
    );
    same(
        |s| {
            s.plan_progress_field_text(FIRST, "actual_start", "2026-01-06")
                .unwrap()
        },
        |s| {
            s.set_progress_field_text(FIRST, "actual_start", "2026-01-06")
                .unwrap();
        },
    );
}

#[test]
fn a_plan_that_changes_nothing_is_none_and_a_refused_one_changes_nothing() {
    let s = session();
    assert_eq!(s.plan_finish_text(FIRST, "2026-01-05"), Ok(None));
    assert!(s.plan_start_text(FIRST, "soon").is_err());
    assert!(s.plan_percent_text(FIRST, "101").is_err());
    assert_eq!(s.history_len(), (0, 0));
}

#[test]
fn a_progress_plan_refuses_a_finish_before_the_start() {
    let s = session();
    let before = s.solution().clone();
    let plan = s.plan_progress_text(FIRST, "2026-01-10", "2026-01-06", "");
    assert_eq!(plan, Err(EditError::FinishBeforeStart));
    assert_eq!((*s.solution() == before, s.history_len()), (true, (0, 0)));
}

#[test]
fn a_progress_field_plan_refuses_a_finish_before_the_start() {
    let mut s = session();
    s.set_progress_text(FIRST, "2026-01-08", "", "").unwrap();
    let (before, history) = (s.solution().clone(), s.history_len());
    let plan = s.plan_progress_field_text(FIRST, "actual_finish", "2026-01-07");
    assert_eq!(plan, Err(EditError::FinishBeforeStart));
    assert_eq!((*s.solution() == before, s.history_len()), (true, history));
}
