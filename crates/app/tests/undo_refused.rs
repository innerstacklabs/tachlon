//! Undo and Redo drop a step the engine refuses about the rows, and go on (tl#2087); `Session::apply` records nothing.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::reparent::build;
use app::{
    Place, Session,
    edit::{Edit, EditError},
    host::KeyTarget,
};
use engine::{DepId, LinkType, TaskId};

const A: TaskId = TaskId(1);
const E: TaskId = TaskId(5);
const N1: &str = r#"{"key":"n1"}"#;

/// P / A, B under it / C, D, E: links B to C, C to D and B to D; E is renamed `e1`.
fn built() -> Session {
    let rows = [("P", None, 1), ("A", Some(0), 2), ("B", Some(0), 2)];
    let rows = rows
        .into_iter()
        .chain([("C", None, 1), ("D", None, 1), ("E", None, 1)]);
    let mut s = build(&rows.collect::<Vec<_>>(), &[(2, 3), (3, 4), (2, 4)]);
    let name = "e1".into();
    s.edit(Edit::Rename { task: E, name }).unwrap();
    s
}

fn below(s: &mut Session, task: TaskId) -> TaskId {
    s.select_task(task.0);
    s.insert_below_selection().unwrap()
}

fn set_progress(s: &mut Session, task: TaskId) {
    let progress = Edit::SetProgress {
        task,
        actual_start: Some(0),
        actual_finish: None,
        remaining: Some(30),
    };
    s.edit(progress).unwrap();
}

/// A child under `task` with no history step, so `task` is a summary.
fn give_child(s: &mut Session, task: TaskId) {
    let place = Place {
        parent: Some(task),
        after: None,
    };
    s.apply(Edit::Insert {
        place,
        name: "k".into(),
    })
    .unwrap();
}

/// Press Undo until it answers false; the number of presses that undid a step.
fn undo_all(s: &mut Session) -> usize {
    let mut presses = 0;
    while s.undo().unwrap() {
        presses += 1;
    }
    presses
}

/// One Undo drops the refused step, undoes the rename under it and leaves it on the redo list only.
fn assert_goes_on(s: &mut Session) {
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.meta()[E.0 as usize].name, "E");
    assert_eq!(s.history_len(), (0, 1));
}

#[test]
fn an_undo_goes_past_the_delete_of_a_row_an_outdent_left_a_child() {
    let mut s = built();
    let g = below(&mut s, E);
    let x = below(&mut s, g);
    s.select_task(x.0);
    s.indent_selected().unwrap();
    let p = below(&mut s, x);
    s.assign_key(KeyTarget::Task(p), "x").unwrap();
    s.select_task(p.0);
    s.outdent_selected().unwrap();
    s.add_task_by_key(N1, Some("x"), None, false).unwrap();
    assert_eq!(undo_all(&mut s), 4, "the delete of G is dropped on the way");
    assert_eq!(s.meta()[E.0 as usize].name, "E");
}

#[test]
fn an_undo_goes_past_a_delete_left_when_an_add_forgot_an_outdent() {
    let mut s = built();
    let g = below(&mut s, E);
    let r = below(&mut s, g);
    s.select_task(r.0);
    s.indent_selected().unwrap();
    let y = below(&mut s, r);
    s.assign_key(KeyTarget::Task(y), "y").unwrap();
    s.select_task(y.0);
    s.indent_selected().unwrap();
    below(&mut s, y);
    s.select_task(y.0);
    s.outdent_selected().unwrap();
    s.add_task_by_key(N1, Some("y"), None, false).unwrap();
    undo_all(&mut s);
    assert_eq!(s.meta()[E.0 as usize].name, "E");
}

#[test]
fn a_delete_of_a_row_that_gained_a_child_is_dropped_and_the_press_goes_on() {
    let mut s = built();
    let g = below(&mut s, E);
    give_child(&mut s, g);
    assert_goes_on(&mut s);
}

#[test]
fn a_link_step_onto_a_new_summary_is_dropped_and_the_press_goes_on() {
    let mut s = built();
    let link = Edit::AddLink {
        pred: E,
        succ: TaskId(4),
        kind: LinkType::FS,
        lag: 0,
    };
    s.edit(link).unwrap();
    s.edit(Edit::RemoveLink { dep: DepId(3) }).unwrap();
    give_child(&mut s, E);
    assert_goes_on(&mut s);
}

#[test]
fn progress_on_a_new_summary_is_dropped_and_a_press_with_nothing_left_says_so() {
    let mut s = built();
    s.clear_history();
    set_progress(&mut s, A);
    give_child(&mut s, A);
    let texts = r#"{"stepGone":"Ein Schritt ging verloren"}"#;
    let locale = format!(
        r#"{{"months":{:?},"weekdays":{:?},"weekStart":0,"texts":{texts}}}"#,
        ["m"; 12], ["d"; 7]
    );
    s.set_locale(&locale).unwrap();
    let error = s.undo().unwrap_err();
    assert_eq!(error.kind(), "stepGone");
    assert_eq!(s.say(&error), "Ein Schritt ging verloren");
    assert_eq!((s.history_len(), s.undo()), ((0, 0), Ok(false)));
}

#[test]
fn a_redo_step_the_engine_refuses_is_dropped_too() {
    let mut s = built();
    set_progress(&mut s, A);
    assert_eq!(s.undo(), Ok(true));
    give_child(&mut s, A);
    assert_eq!(s.redo().unwrap_err().kind(), "stepGone");
}

#[test]
fn a_view_refusal_returns_and_the_step_stays_until_the_view_is_off() {
    let mut s = built();
    s.select_task(2);
    s.indent_selected().unwrap();
    assert!(s.set_critical_only(true));
    assert_eq!(s.undo(), Err(EditError::ReparentFiltered));
    assert_eq!(s.history_len(), (2, 0));
    assert!(s.set_critical_only(false));
    assert_eq!(s.undo(), Ok(true));
    assert!(s.set_critical_only(true));
    assert_eq!(s.redo(), Err(EditError::ReparentFiltered));
    assert_eq!(s.history_len(), (1, 1));
    assert!(s.set_critical_only(false));
    assert_eq!(s.redo(), Ok(true));
}

#[test]
fn two_dropped_steps_then_an_applied_one_report_once_and_carry_two() {
    let mut s = built();
    set_progress(&mut s, A);
    set_progress(&mut s, E);
    give_child(&mut s, A);
    give_child(&mut s, E);
    let texts = r#"{"stepGoneMany":"{count} Schritte gingen verloren"}"#;
    let locale = format!(
        r#"{{"months":{:?},"weekdays":{:?},"weekStart":0,"texts":{texts}}}"#,
        ["m"; 12], ["d"; 7]
    );
    s.set_locale(&locale).unwrap();

    let pressed = s.undo_counted().unwrap();
    assert!(pressed.applied);
    assert_eq!(pressed.dropped, 2);
    assert_eq!(
        pressed.report,
        Some("2 Schritte gingen verloren".to_string())
    );
    assert_eq!(s.meta()[E.0 as usize].name, "E");

    let pressed = s.redo_counted().unwrap();
    assert!(pressed.applied);
    assert_eq!(pressed.dropped, 0);
    assert_eq!(pressed.report, None);
}

#[test]
fn a_press_that_drops_nothing_reports_nothing_and_carries_zero() {
    let mut s = built();
    let pressed = s.undo_counted().unwrap();
    assert!(pressed.applied);
    assert_eq!(pressed.dropped, 0);
    assert_eq!(pressed.report, None);

    let pressed = s.redo_counted().unwrap();
    assert!(pressed.applied);
    assert_eq!(pressed.dropped, 0);
    assert_eq!(pressed.report, None);
}

#[test]
fn a_press_with_nothing_left_still_answers_step_gone_through_the_counted_press() {
    let mut s = built();
    s.clear_history();
    set_progress(&mut s, A);
    give_child(&mut s, A);
    assert_eq!(s.undo_counted().unwrap_err().kind(), "stepGone");
}

#[test]
fn a_press_that_drops_steps_hands_its_report_to_the_alert_line_once() {
    let mut s = built();
    set_progress(&mut s, A);
    set_progress(&mut s, E);
    give_child(&mut s, A);
    give_child(&mut s, E);
    assert_eq!(s.link_refusal(), None);

    assert_eq!(s.undo(), Ok(true));
    let report =
        "2 steps could no longer be undone or redone, so the chart dropped them from the list.";
    assert_eq!(s.link_refusal(), Some(report));

    assert_eq!(s.redo(), Ok(true));
    assert_eq!(
        s.link_refusal(),
        None,
        "a press that drops none hands over none"
    );
}

#[test]
fn a_baseline_cleared_outside_the_history_does_not_stick_undo() {
    let mut s = built();
    s.edit(Edit::SetBaseline { baseline: None }).unwrap();
    s.apply(Edit::ClearBaseline).unwrap();

    let pressed = s.undo_counted().unwrap();
    assert!(pressed.applied);
    assert_eq!(pressed.dropped, 1);
    assert_eq!(s.meta()[E.0 as usize].name, "E");
    assert_eq!(s.history_len(), (0, 1));
}
