//! The host capability profile (gantt#2090, decision 0019, rule 5): a gesture or a group of fields the
//! host turned off is refused with `EditError::NotAllowed` on every path, leaves the chart as it was,
//! and the rest of the chart still works.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::{link, on, session};
use crate::read_only::{longer, state};
use app::dates::Origin;
use app::{Session, edit::EditError};
use engine::TaskId;
use std::{cell::Cell, rc::Rc};

const NO_STRUCTURE: &str = r#"{"editable":{"structure":false}}"#;
const ROWS: EditError = EditError::NotAllowed("adding, deleting and moving rows");

fn limited(json: &str) -> Session {
    let mut s = session();
    s.set_capabilities(json).unwrap();
    s
}

/// A press on task 2's bar, or on its finish edge, dragged far to the right: whether it changed the
/// chart, and the note it left while the pointer was still down.
fn dragged(json: &str, edge: bool) -> (bool, Option<String>) {
    let mut s = limited(json);
    let (before, (x, y)) = (state(&s), on(&s, 2));
    let x = if edge { x + 174.0 } else { x };
    s.pointer_down(x, y, false);
    s.pointer_move(x + 1500.0, y);
    s.apply_drag();
    let note = s.drag_note().map(str::to_string);
    s.pointer_up();
    (state(&s) != before, note)
}

#[test]
fn structure_off_refuses_rows_and_paste_and_lets_progress_through() {
    let mut s = limited(NO_STRUCTURE);
    let before = state(&s);
    s.select(1);
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    assert_eq!(s.delete_selected().map(drop), Err(ROWS));
    s.select(5);
    assert_eq!(s.outdent_selected().map(drop), Err(ROWS));
    assert_eq!(s.insert_many(&["Footing"]).map(drop), Err(ROWS));
    assert_eq!(state(&s), before, "a refusal changes nothing");
    s.set_progress_field_text(TaskId(1), "actual_start", "2026-01-05")
        .unwrap();
    s.set_task_fields(TaskId(0), &[("percent", "50")]).unwrap();
    assert_ne!(state(&s), before, "progress still edits");
}

#[test]
fn bar_move_off_refuses_a_move_and_still_resizes() {
    let json = r#"{"gestures":{"barMove":false}}"#;
    let said = EditError::NotAllowed("moving a bar").to_string();
    assert_eq!(dragged(json, false), (false, Some(said)));
    assert_eq!(dragged(json, true), (true, None));
}

#[test]
fn resize_off_refuses_an_edge_drag_and_still_moves() {
    let json = r#"{"gestures":{"resize":false}}"#;
    let said = EditError::NotAllowed("resizing a bar").to_string();
    assert_eq!(dragged(json, true), (false, Some(said)));
    assert_eq!(dragged(json, false), (true, None));
}

#[test]
fn link_off_adds_no_link_and_the_press_still_selects() {
    let mut s = limited(r#"{"gestures":{"link":false}}"#);
    let (before, to) = (state(&s), on(&s, 2));
    link(&mut s, 0, to);
    assert_eq!(state(&s), before);
    assert_eq!(s.selection().map(|selected| selected.task), Some(0));
    let said = EditError::NotAllowed("drawing a link").to_string();
    assert_eq!(s.drag_note(), Some(said.as_str()));
}

#[test]
fn dates_off_refuses_a_dialog_whole_and_names_the_duration() {
    let mut s = limited(r#"{"editable":{"dates":false}}"#);
    let before = state(&s);
    let fields = [("name", "Renamed"), ("duration", "5h")];
    let said = s.set_task_fields(TaskId(0), &fields).unwrap_err();
    assert_eq!(
        said,
        "The host does not allow changing dates and durations."
    );
    assert_eq!((state(&s), s.meta()[0].name.as_str()), (before, "A"));
    s.set_task_fields(TaskId(0), &fields[..1]).unwrap();
    assert_eq!(s.meta()[0].name, "Renamed", "the name alone passes");
}

#[test]
fn undo_of_a_delete_is_refused_with_structure_off() {
    let mut s = session();
    s.select(1);
    s.delete_selected().unwrap();
    s.set_capabilities(NO_STRUCTURE).unwrap();
    let before = state(&s);
    assert_eq!(s.undo(), Err(ROWS));
    assert_eq!(state(&s), before);
    s.set_capabilities("{}").unwrap();
    assert_eq!(s.undo(), Ok(true), "off again, the delete undoes");
}

#[test]
fn undo_of_a_data_date_is_refused_with_progress_off() {
    let mut s = session();
    s.set_data_date_text("2026-01-05").unwrap();
    s.set_capabilities(r#"{"editable":{"progress":false}}"#)
        .unwrap();
    let before = state(&s);
    let refused = EditError::NotAllowed("changing progress and the data date");
    assert_eq!(s.undo(), Err(refused));
    assert_eq!(state(&s), before, "the date stays");
}

#[test]
fn read_only_refuses_first_and_the_host_function_is_not_asked_after_a_refusal() {
    let asked = Rc::new(Cell::new(0));
    let count = Rc::clone(&asked);
    let mut s = limited(NO_STRUCTURE);
    s.set_before_edit(Some(Rc::new(move |_| {
        count.set(count.get() + 1);
        Ok(())
    })));
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    assert_eq!(asked.get(), 0, "a profile refusal never reaches it");
    s.edit(longer(300)).unwrap();
    assert_eq!(asked.get(), 1, "an allowed edit does");
    s.set_read_only(true);
    assert_eq!(
        s.insert_below_selection().map(drop),
        Err(EditError::ReadOnly)
    );
}

#[test]
fn a_profile_that_is_not_one_is_refused_by_name_and_changes_nothing() {
    let mut s = limited(NO_STRUCTURE);
    for (json, said) in [
        (
            r#"{"gestures":{"drag":false}}"#,
            "gestures.drag: unknown key",
        ),
        (r#"{"edit":{}}"#, "edit: unknown key"),
        (
            r#"{"editable":{"dates":0}}"#,
            "editable.dates: expected true or false",
        ),
        ("[]", "expected an object, found an array"),
    ] {
        assert_eq!(s.set_capabilities(json), Err(said.to_string()), "{json}");
    }
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
}

#[test]
fn no_profile_allows_everything_and_an_empty_one_takes_a_profile_away() {
    let mut s = session();
    s.insert_below_selection().unwrap();
    s.set_capabilities(NO_STRUCTURE).unwrap();
    assert_eq!(s.insert_below_selection().map(drop), Err(ROWS));
    s.set_capabilities("{}").unwrap();
    s.insert_below_selection().unwrap();
}

#[test]
fn reopen_keeps_the_profile() {
    let s = limited(NO_STRUCTURE);
    let meta = s.meta().to_vec();
    let order = (0..meta.len() as u32).collect();
    let (schedule, solution) = (s.schedule().clone(), s.solution().clone());
    let fresh = s.reopen(schedule, solution, order, meta, None, None, Origin::DEMO);
    assert_eq!(fresh.unwrap().insert_below_selection().map(drop), Err(ROWS));
}
