//! Read-only mode (gantt#2085, decision 0019, rule 3): with it on, every way to change the schedule
//! is refused with `EditError::ReadOnly` and leaves the schedule, the revision and the history as
//! they were; a click on a row still selects; with it off each path works again.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::{link, on, session};
use app::dates::Origin;
use app::history::Source;
use app::{Session, WhatIfError, edit::Edit, edit::EditError};
use engine::{DepId, TaskId};

type Case = (&'static str, fn(&mut Session) -> Result<(), EditError>);

pub(crate) fn longer(minutes: i64) -> Edit {
    Edit::SetDuration {
        task: TaskId(0),
        minutes,
    }
}

/// Two steps, one of them undone, and a what-if mark between them: undo, redo and a discard all have work.
pub(crate) fn with_history() -> Session {
    let mut s = session();
    s.edit(Edit::RemoveLink { dep: DepId(0) }).unwrap();
    s.edit(longer(120)).unwrap();
    s.begin_whatif().unwrap();
    s.edit(longer(180)).unwrap();
    s.edit(longer(240)).unwrap();
    assert!(s.undo().unwrap());
    s
}

fn cases() -> [Case; 12] {
    [
        ("a typed start", |s| {
            let (task, snet) = (TaskId(0), Some(600));
            s.edit(Edit::Move { task, snet })
        }),
        ("a typed duration", |s| s.edit(longer(300))),
        ("an accepted proposal", |s| {
            s.edit_group(vec![longer(300)], Source::Proposal)
        }),
        ("the dialog", |s| {
            let said = s.set_task_fields(TaskId(0), &[("duration", "5h")]);
            said.map_err(|text| {
                assert_eq!(text, EditError::ReadOnly.to_string());
                EditError::ReadOnly
            })
        }),
        ("paste", |s| s.insert_many(&["Footing"]).map(drop)),
        ("undo", |s| s.undo().map(drop)),
        ("redo", |s| s.redo().map(drop)),
        ("a what-if discard", |s| {
            s.discard_whatif().map_err(|error| match error {
                WhatIfError::Undo(error) => error,
                other => panic!("the discard failed for another reason: {other:?}"),
            })
        }),
        ("the keyboard's insert", |s| {
            s.insert_below_selection().map(drop)
        }),
        ("the keyboard's delete", |s| {
            s.select(1);
            s.delete_selected().map(drop)
        }),
        ("the keyboard's indent", |s| {
            s.select(1);
            s.indent_selected().map(drop)
        }),
        ("the keyboard's outdent", |s| {
            s.select(5);
            s.outdent_selected().map(drop)
        }),
    ]
}

/// What a refusal must leave alone.
pub(crate) fn state(s: &Session) -> impl PartialEq + std::fmt::Debug + use<> {
    let schedule = s.schedule();
    (
        s.revision(),
        s.history_len(),
        s.solution().dates.clone(),
        schedule.tasks().to_vec(),
        schedule.live_dependencies().count(),
    )
}

fn read_only(mut s: Session) -> Session {
    s.set_read_only(true);
    s
}

#[test]
fn each_edit_path_is_refused_and_leaves_the_chart_as_it_was() {
    for (name, act) in cases() {
        let mut s = read_only(with_history());
        let before = state(&s);
        assert_eq!(act(&mut s), Err(EditError::ReadOnly), "{name}");
        assert_eq!(state(&s), before, "{name} changed the chart");
        s.set_read_only(false);
        assert_eq!(act(&mut s), Ok(()), "{name} with read-only off");
        assert_ne!(state(&s), before, "{name} changed nothing");
    }
}

#[test]
fn indent_outdent_and_move_say_read_only_on_a_row_they_would_refuse_for_another_reason() {
    let mut s = read_only(session());
    s.select(0);
    let before = state(&s);
    assert_eq!(s.indent_selected(), Err(EditError::ReadOnly));
    assert_eq!(s.outdent_selected(), Err(EditError::ReadOnly));
    assert_eq!(s.move_selected_up(), Err(EditError::ReadOnly));
    assert_eq!(state(&s), before);
}

#[test]
fn a_bar_drag_and_an_edge_drag_past_the_threshold_move_nothing_and_say_why() {
    let sentence = EditError::ReadOnly.to_string();
    for edge in [false, true] {
        let mut s = read_only(session());
        let (x, y) = on(&s, 2);
        let x = if edge { x + 174.0 } else { x };
        let before = state(&s);
        s.pointer_down(x, y, false);
        s.pointer_move(x + 1500.0, y);
        s.apply_drag();
        assert_eq!(state(&s), before, "no frame moves the bar");
        assert_eq!(s.drag_note(), Some(sentence.as_str()));
        s.pointer_up();
        assert_eq!(state(&s), before);
        s.set_read_only(false);
        s.pointer_down(x, y, false);
        s.pointer_move(x + 1500.0, y);
        s.pointer_up();
        assert_ne!(state(&s), before, "off again, the drag edits");
    }
}

#[test]
fn a_click_still_selects_and_a_press_below_the_threshold_edits_nothing() {
    let mut s = read_only(session());
    let before = state(&s);
    let (x, y) = on(&s, 2);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 2.0, y);
    s.pointer_up();
    assert_eq!(s.selection().map(|selected| selected.task), Some(2));
    assert_eq!((state(&s), s.drag_note()), (before, None));
}

#[test]
fn a_link_drag_adds_no_link_and_the_press_on_the_terminal_still_selects() {
    let mut s = read_only(session());
    let (before, to) = (state(&s), on(&s, 2));
    link(&mut s, 0, to);
    assert_eq!(state(&s), before);
    assert_eq!(s.selection().map(|selected| selected.task), Some(0));
    assert_eq!(
        s.drag_note(),
        Some(EditError::ReadOnly.to_string().as_str())
    );
    s.set_read_only(false);
    link(&mut s, 0, to);
    assert_ne!(state(&s), before, "off again, the drag adds the link");
}

#[test]
fn a_link_drag_started_before_the_switch_adds_nothing_at_the_release() {
    let mut s = session();
    let (before, from, to) = (state(&s), on(&s, 0), on(&s, 2));
    s.pointer_down(from.0, from.1, true);
    s.pointer_move(to.0, to.1);
    s.set_read_only(true);
    s.pointer_up();
    assert_eq!(state(&s), before);
    assert!(
        s.link_refusal()
            .is_some_and(|text| text.contains("read-only"))
    );
}

#[test]
fn reopen_keeps_the_flag() {
    let s = read_only(session());
    let meta = s.meta().to_vec();
    let order = (0..meta.len() as u32).collect();
    let (schedule, solution) = (s.schedule().clone(), s.solution().clone());
    let fresh = s.reopen(schedule, solution, order, meta, None, None, Origin::DEMO);
    assert!(fresh.unwrap().read_only());
}
