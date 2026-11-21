//! A step the engine refuses when Undo or Redo applies it is dropped and the press goes on (tl#2087):
//! Undo never sticks. `Session::apply` records nothing, so a test changes the rows under a list by it.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::reparent::build;
use app::edit::Edit;
use app::{Place, Session, host::KeyTarget};
use engine::{DepId, LinkType, TaskId};

const A: TaskId = TaskId(1);
const E: TaskId = TaskId(5);

/// P / A, B under it / C, D, E: links B to C, C to D and B to D, and each task keyed `t<slot>`.
fn built() -> Session {
    let rows = [
        ("P", None, 1),
        ("A", Some(0), 2),
        ("B", Some(0), 2),
        ("C", None, 1),
        ("D", None, 1),
        ("E", None, 1),
    ];
    let mut s = build(&rows, &[(2, 3), (3, 4), (2, 4)]);
    for n in 0..6 {
        s.assign_key(KeyTarget::Task(TaskId(n)), &format!("t{n}"))
            .unwrap();
    }
    s
}

fn rename_e(s: &mut Session) {
    let name = "e1".into();
    s.edit(Edit::Rename { task: E, name }).unwrap();
}

fn set_progress(s: &mut Session, task: TaskId) {
    s.edit(Edit::SetProgress {
        task,
        actual_start: Some(0),
        actual_finish: None,
        remaining: Some(30),
    })
    .unwrap();
}

/// A child under `task`, put there with no history step, so `task` is a summary.
fn give_child(s: &mut Session, task: TaskId) {
    let place = Place {
        parent: Some(task),
        after: None,
    };
    let name = "k".into();
    s.apply(Edit::Insert { place, name }).unwrap();
}

/// Press Undo until it answers false; the number of presses that undid a step.
fn undo_all(s: &mut Session) -> usize {
    let mut presses = 0;
    while s.undo().unwrap() {
        presses += 1;
        assert!(presses < 20, "Undo does not end");
    }
    presses
}

#[test]
fn an_undo_goes_past_the_delete_of_a_row_an_outdent_left_a_child() {
    let mut s = built();
    rename_e(&mut s);
    s.select_task(E.0);
    let g = s.insert_below_selection().unwrap();
    s.select_task(g.0);
    let x = s.insert_below_selection().unwrap();
    s.select_task(x.0);
    s.indent_selected().unwrap();
    s.select_task(x.0);
    let p = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(p), "x").unwrap();
    s.select_task(p.0);
    s.outdent_selected().unwrap();
    s.add_task_by_key(r#"{"key":"n1"}"#, Some("x"), None)
        .unwrap();
    assert_eq!(undo_all(&mut s), 4, "the delete of G is dropped on the way");
    assert_eq!(s.meta()[E.0 as usize].name, "E");
}

#[test]
fn an_undo_goes_past_a_delete_left_over_when_an_add_forgot_an_outdent() {
    let mut s = built();
    rename_e(&mut s);
    s.select_task(E.0);
    let g = s.insert_below_selection().unwrap();
    s.select_task(g.0);
    let r = s.insert_below_selection().unwrap();
    s.select_task(r.0);
    s.indent_selected().unwrap();
    s.select_task(r.0);
    let y = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(y), "y").unwrap();
    s.select_task(y.0);
    s.indent_selected().unwrap();
    s.select_task(y.0);
    s.insert_below_selection().unwrap();
    s.select_task(y.0);
    s.outdent_selected().unwrap();
    s.add_task_by_key(r#"{"key":"n1"}"#, Some("y"), None)
        .unwrap();
    undo_all(&mut s);
    assert_eq!(s.meta()[E.0 as usize].name, "E");
}

#[test]
fn a_delete_of_a_row_that_gained_a_child_is_dropped_and_the_press_goes_on() {
    let mut s = built();
    rename_e(&mut s);
    s.select_task(E.0);
    let g = s.insert_below_selection().unwrap();
    give_child(&mut s, g);
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.meta()[E.0 as usize].name, "E");
    assert_eq!(s.history_len(), (0, 1), "the dropped step is on no list");
}

#[test]
fn progress_on_a_new_summary_is_dropped_and_a_press_with_nothing_left_says_so() {
    let mut s = built();
    set_progress(&mut s, A);
    give_child(&mut s, A);
    let months = r#"["a","b","c","d","e","f","g","h","i","j","k","l"]"#;
    let days = r#"["a","b","c","d","e","f","g"]"#;
    let texts = r#"{"stepGone":"Ein Schritt ging verloren"}"#;
    let locale =
        format!(r#"{{"months":{months},"weekdays":{days},"weekStart":0,"texts":{texts}}}"#);
    s.set_locale(&locale).unwrap();
    let error = s.undo().unwrap_err();
    assert_eq!(error.kind(), "stepGone");
    assert_eq!(s.say(&error), "Ein Schritt ging verloren");
    assert_eq!(s.history_len(), (0, 0));
    assert_eq!(
        s.undo(),
        Ok(false),
        "the press that said so is the only one"
    );
}

#[test]
fn a_link_step_onto_a_new_summary_is_dropped_and_the_press_goes_on() {
    let mut s = built();
    rename_e(&mut s);
    let link = Edit::AddLink {
        pred: E,
        succ: TaskId(4),
        kind: LinkType::FS,
        lag: 0,
    };
    s.edit(link).unwrap();
    s.edit(Edit::RemoveLink { dep: DepId(3) }).unwrap();
    give_child(&mut s, E);
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.meta()[E.0 as usize].name, "E");
    assert_eq!(s.history_len(), (0, 1));
}

#[test]
fn a_redo_step_the_engine_refuses_is_dropped_too() {
    let mut s = built();
    set_progress(&mut s, A);
    assert_eq!(s.undo(), Ok(true));
    give_child(&mut s, A);
    assert_eq!(s.redo().unwrap_err().kind(), "stepGone");
    assert_eq!(s.history_len(), (0, 0));
}
