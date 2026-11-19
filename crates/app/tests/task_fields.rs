//! A task's changed fields as one step (gantt#1875): `Session::set_task_fields` applies the dialog's
//! edits through one undo step and names the field it refused. The chain of `start_edit`: three
//! tasks of eight working hours, day 0 is Monday 2026-01-05.

use crate::start_edit::session;
use app::Session;

/// Set the fields of the first task.
fn set(s: &mut Session, fields: &[(&str, &str)]) -> Result<(), String> {
    s.set_task_fields(engine::TaskId(0), fields)
}

/// The task as the session started: the solution, the name and nothing recorded.
fn untouched(s: &Session) {
    assert_eq!(*s.solution(), *session().solution());
    assert_eq!(s.meta()[0].name, "");
    assert_eq!(s.history_len(), (0, 0), "nothing recorded");
}

#[test]
fn a_name_a_duration_and_a_start_are_one_undo_step() {
    let mut s = session();
    set(
        &mut s,
        &[
            ("name", " Pour "),
            ("duration", "16h"),
            ("start", "2026-01-12"),
        ],
    )
    .unwrap();
    assert_eq!(s.meta()[0].name, "Pour");
    let task = &s.schedule().tasks()[0];
    assert_eq!((task.duration, task.snet), (16 * 60, Some(7 * 24 * 60)));
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
    assert_eq!(s.history_len(), (1, 0), "one step, not three");
    assert!(s.undo().unwrap());
    assert_eq!(s.history_len(), (0, 1));
    assert_eq!(*s.solution(), *session().solution());
    assert_eq!(s.meta()[0].name, "");
}

#[test]
fn a_bad_duration_names_duration_and_changes_nothing() {
    let mut s = session();
    let error = set(
        &mut s,
        &[
            ("name", "Pour"),
            ("duration", "soon"),
            ("start", "2026-01-12"),
        ],
    );
    assert!(error.unwrap_err().starts_with("Duration: not a duration"));
    untouched(&s);
}

#[test]
fn a_blank_name_is_refused_through_the_group() {
    let mut s = session();
    let error = set(&mut s, &[("duration", "16h"), ("name", "  ")]).unwrap_err();
    assert!(error.starts_with("Name: "), "{error}");
    untouched(&s);
}

#[test]
fn a_percent_with_a_duration_is_refused() {
    let mut s = session();
    let error = set(&mut s, &[("duration", "16h"), ("percent", "50%")]).unwrap_err();
    assert!(error.starts_with("Percent: "), "{error}");
    untouched(&s);
}

#[test]
fn a_percent_alone_and_two_progress_fields_keep_each_other() {
    let mut s = session();
    set(&mut s, &[("percent", "50")]).unwrap();
    assert_eq!(s.schedule().tasks()[0].remaining, Some(4 * 60));
    set(
        &mut s,
        &[("actual_start", "2026-01-06"), ("remaining", "2h")],
    )
    .unwrap();
    let task = &s.schedule().tasks()[0];
    assert_eq!(
        (task.actual_start, task.remaining),
        (Some(24 * 60), Some(2 * 60))
    );
    assert_eq!(s.history_len(), (2, 0));
}

#[test]
fn the_change_summary_is_the_one_the_duration_makes() {
    let mut s = session();
    set(&mut s, &[("duration", "16h"), ("name", "Pour")]).unwrap();
    let report = s.move_report().expect("two tasks moved");
    assert_eq!(report.text, "2 tasks moved · finish +1 day");
}

#[test]
fn a_start_and_a_finish_together_equal_the_two_edits_in_turn() {
    let mut s = session();
    let both = [("start", "2026-01-12"), ("finish", "2026-01-13")];
    set(&mut s, &both).unwrap();
    let mut turn = session();
    let task = engine::TaskId(0);
    turn.set_start_text(task, "2026-01-12").unwrap();
    turn.set_finish_text(task, "2026-01-13").unwrap();
    assert_eq!(s.schedule().tasks()[0].duration, 16 * 60, "Jan 12 and 13");
    assert_eq!(s.schedule().tasks()[0], turn.schedule().tasks()[0]);
    assert_eq!(*s.solution(), *turn.solution());
    assert_eq!(s.history_len(), (1, 0), "one step, not two");
}

#[test]
fn a_refusal_after_a_field_applied_puts_the_task_back() {
    let mut s = session();
    let fields = [
        ("name", "Pour"),
        ("start", "2026-01-12"),
        ("finish", "2026-01-09"),
    ];
    let error = set(&mut s, &fields).unwrap_err();
    assert!(error.starts_with("Finish: "), "{error}");
    untouched(&s);
}

/// Two tasks with the Activity IDs `1` and `2` and no link, and what the dialog sends for the second.
fn with_ids(fields: &[(&str, &str)]) -> (Session, Result<(), String>) {
    let tasks = [("A", None, 60), ("B", None, 60)];
    let mut s = crate::links::build_with(&tasks, &[], |meta| {
        (meta[0].code, meta[1].code) = (Some("1".into()), Some("2".into()));
    });
    let result = s.set_task_fields(engine::TaskId(1), fields);
    (s, result)
}

#[test]
fn a_name_and_a_predecessors_list_are_one_undo_step() {
    let (mut s, result) = with_ids(&[("name", "Pour"), ("predecessors", "1SS+1d")]);
    result.unwrap();
    assert_eq!(s.predecessors_text(engine::TaskId(1)).unwrap(), "1 SS+1d");
    assert_eq!(s.history_len(), (1, 0), "one step, not two");
    assert!(s.undo().unwrap());
    assert_eq!(s.meta()[1].name, "B");
    assert_eq!(s.predecessors_text(engine::TaskId(1)).unwrap(), "");
}

#[test]
fn a_bad_predecessors_list_names_predecessors_and_changes_nothing() {
    let (s, result) = with_ids(&[("name", "Pour"), ("predecessors", "9")]);
    assert!(result.unwrap_err().starts_with("Predecessors: "));
    assert_eq!((s.meta()[1].name.as_str(), s.history_len()), ("B", (0, 0)));
}
