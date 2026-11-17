//! A task's date limits through `set_task_fields` (gantt#1875), on the chain of `start_edit`.

use crate::start_edit::session;
use engine::{Limit, TaskId};

#[test]
fn all_three_limits_and_a_duration_are_one_undo_step() {
    let mut s = session();
    let fields = [
        ("duration", "16h"),
        ("snlt", "2026-01-05"),
        ("fnet", "2026-01-08"),
        ("fnlt", "2026-01-05"),
    ];
    s.set_task_fields(TaskId(0), &fields).unwrap();
    let l = s.schedule().tasks()[0].limit;
    assert_eq!(
        (l.snlt, l.fnet, l.fnlt),
        (Some(0), Some(4 * 1440), Some(1440))
    );
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
    assert_eq!(s.history_len(), (1, 0));
    assert!(s.undo().unwrap());
    assert_eq!(s.schedule().tasks()[0].limit, Limit::default());
    assert_eq!(*s.solution(), *session().solution());
}

#[test]
fn a_blank_clears_one_limit_and_keeps_the_others() {
    let mut s = session();
    let limits = [("snlt", "2026-01-12"), ("fnlt", "2026-01-14")];
    s.set_task_fields(TaskId(0), &limits).unwrap();
    s.set_task_fields(TaskId(0), &[("snlt", " ")]).unwrap();
    let limit = s.schedule().tasks()[0].limit;
    assert_eq!((limit.snlt, limit.fnlt), (None, Some(10 * 1440)));
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

#[test]
fn a_bad_day_names_the_field_and_changes_nothing() {
    let mut s = session();
    let bad = [("duration", "16h"), ("fnet", "soon")];
    let error = s.set_task_fields(TaskId(0), &bad).unwrap_err();
    assert!(error.starts_with("Finish no earlier than:"), "{error}");
    assert_eq!(s.history_len(), (0, 0));
    assert_eq!(*s.solution(), *session().solution());
    assert_eq!(s.schedule().tasks()[0].limit, Limit::default());
}
