//! Typing a physical percent (tl#2080): it is stored beside the percent derived from the remaining
//! work, moves no date, runs no solve, and is one undo step. Task 0 of the three-task chain is
//! started with 6 of its 8 hours left, so it reads 25% derived.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::collapse;
use crate::link_gesture::on;
use crate::read_only::state;
use crate::start_edit::session;
use app::{Session, edit::EditError};
use engine::TaskId;
use serde_json::{Value, json};

const FIRST: TaskId = TaskId(0);

fn started() -> Session {
    let mut s = session();
    s.set_progress_text(FIRST, "2026-01-05", "", "6h").unwrap();
    s.take_commits();
    s
}

fn matches_compute(s: &Session) {
    let fresh = s.schedule().clone().compute().unwrap();
    assert_eq!(*s.solution(), fresh);
}

#[test]
fn a_typed_physical_percent_beside_the_derived_one_moves_nothing() {
    let mut s = started();
    let (tasks, dates) = (s.schedule().tasks().to_vec(), s.solution().clone());
    s.set_physical_text(FIRST, "50").unwrap();
    assert_eq!(
        (s.task_physical(0).as_str(), s.task_percent(0).as_str()),
        ("50%", "25%")
    );
    assert_eq!((s.schedule().tasks(), s.solution()), (&tasks[..], &dates));
    matches_compute(&s);
}

#[test]
fn one_undo_puts_the_old_value_back_and_redo_the_new() {
    let mut s = started();
    s.set_physical_text(FIRST, "20").unwrap();
    let steps = s.history_len().0;
    s.set_physical_text(FIRST, "50").unwrap();
    assert_eq!(s.history_len().0, steps + 1);
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.task_physical(0), "20%");
    matches_compute(&s);
    assert_eq!(s.redo(), Ok(true));
    assert_eq!(s.task_physical(0), "50%");
    matches_compute(&s);
}

#[test]
fn the_forms_a_cell_takes_and_empty_text_clears() {
    let mut s = started();
    for (text, shown) in [
        ("50%", "50%"),
        ("12.5", "12.5%"),
        (" 12.35 % ", "12.35%"),
        ("7.50", "7.5%"),
        ("0", "0%"),
        ("100", "100%"),
    ] {
        s.set_physical_text(FIRST, text).unwrap();
        assert_eq!(s.task_physical(0), shown, "{text:?}");
    }
    s.set_physical_text(FIRST, "").unwrap();
    assert_eq!(s.task_physical(0), "");
}

#[test]
fn text_that_is_not_a_percent_with_two_decimals_is_refused_and_changes_nothing() {
    let mut s = started();
    let before = state(&s);
    for text in [
        "101", "1.234", "abc", "-1", "1.", ".5", "1e1", "%", "100.01",
    ] {
        assert_eq!(
            s.set_physical_text(FIRST, text),
            Err(EditError::BadPhysical),
            "{text:?}"
        );
    }
    assert_eq!(state(&s), before);
    assert_eq!(EditError::BadPhysical.kind(), "badPhysical");
}

#[test]
fn a_value_the_task_already_holds_makes_no_undo_step() {
    let mut s = started();
    s.set_physical_text(FIRST, "").unwrap();
    let before = state(&s);
    s.set_physical_text(FIRST, "50").unwrap();
    let after = state(&s);
    s.set_physical_text(FIRST, "50.00").unwrap();
    assert_eq!(state(&s), after);
    assert_ne!(after, before);
}

#[test]
fn a_summary_is_refused() {
    let mut s = collapse::session(60);
    let summary = collapse::summaries(&s)[0];
    let error = s.set_physical_text(summary, "50").unwrap_err();
    assert!(
        matches!(error, EditError::ProgressOnSummary { .. }),
        "{error}"
    );
    assert_eq!(s.history_len().0, 0);
}

#[test]
fn the_host_change_carries_physical_percent_both_ways() {
    let mut s = started();
    let change = |s: &mut Session| -> Value {
        let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
        out["commits"][0]["changes"][0].clone()
    };
    s.set_physical_text(FIRST, "12.5").unwrap();
    let made = change(&mut s);
    assert_eq!(made["field"], "physicalPercent");
    assert_eq!((&made["old"], &made["new"]), (&Value::Null, &json!(12.5)));
    s.undo().unwrap();
    let undone = change(&mut s);
    assert_eq!(
        (&undone["old"], &undone["new"]),
        (&json!(12.5), &Value::Null)
    );
}

#[test]
fn a_host_that_turned_progress_off_refuses_it() {
    let mut s = started();
    s.set_capabilities(r#"{"editable":{"progress":false}}"#)
        .unwrap();
    let before = state(&s);
    let refused = EditError::NotAllowed("changing progress and the data date");
    assert_eq!(s.set_physical_text(FIRST, "50"), Err(refused));
    assert_eq!(state(&s), before);
}

#[test]
fn a_physical_percent_typed_during_a_bar_press_is_refused_and_changes_nothing() {
    let mut s = started();
    let (x, y) = on(&s, 0);
    s.pointer_down(x, y, false);
    let before = (state(&s), s.task_physical(0));
    assert_eq!(s.set_physical_text(FIRST, "50"), Err(EditError::Busy));
    assert_eq!((state(&s), s.task_physical(0)), before);
}
