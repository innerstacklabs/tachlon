//! `rollback_to` puts the chart back to before a commit the host failed to save (decision 0019,
//! rule 11): the commits after it go too, as one commit with source `rollback`.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::{demo_session, outline_session, setup};
use crate::history::snapshot;
use app::{Session, edit::Edit, edit::EditError, host::KeyTarget, meta::TaskMeta};
use engine::TaskId;
use serde_json::Value;

/// Rename the task `task` to `name`: one typed commit.
fn rename(s: &mut Session, task: u32, name: &str) {
    let (task, name) = (TaskId(task), name.into());
    s.edit(Edit::Rename { task, name }).unwrap();
}

/// The first task with no task under it in the outline: a delete takes it alone.
fn leaf_of(meta: &[TaskMeta]) -> TaskId {
    let at = (0..meta.len() - 1).find(|&i| meta[i + 1].outline_level <= meta[i].outline_level);
    TaskId(u32::try_from(at.unwrap()).unwrap())
}

/// The `source` of each commit read from the outbox, oldest first.
fn sources(s: &mut Session) -> Vec<String> {
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    let source = |c: &Value| c["source"].as_str().unwrap().to_string();
    out["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(source)
        .collect()
}

#[test]
fn a_rollback_to_the_second_commit_leaves_the_state_after_the_first_with_one_undo_and_no_redo() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    let after_first = snapshot(&s);
    rename(&mut s, 2, "two");
    rename(&mut s, 3, "three");
    assert_eq!(s.rollback_to(2), Ok(4));
    assert_eq!(snapshot(&s), after_first);
    assert_eq!(s.history_len(), (1, 0));
}

#[test]
fn a_rollback_across_an_undo_and_a_redo_restores_both_stacks() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    rename(&mut s, 2, "two");
    let after_second = snapshot(&s);
    assert!(s.undo().unwrap() && s.redo().unwrap());
    assert_eq!(s.rollback_to(3), Ok(5));
    assert_eq!(snapshot(&s), after_second);
    assert_eq!(s.history_len(), (2, 0));
}

#[test]
fn a_redo_entry_from_before_a_normal_commit_is_not_brought_back() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    rename(&mut s, 2, "two");
    assert!(s.undo().unwrap());
    let after_undo = snapshot(&s);
    rename(&mut s, 3, "three");
    assert_eq!(s.rollback_to(4), Ok(5));
    assert_eq!(snapshot(&s), after_undo);
    assert_eq!(s.history_len(), (1, 0));
}

#[test]
fn a_deleted_task_comes_back_in_its_slot_with_its_key() {
    let (mut s, meta) = outline_session(30);
    let leaf = leaf_of(&meta);
    s.assign_key(KeyTarget::Task(leaf), "k29").unwrap();
    let before = snapshot(&s);
    s.select_task(leaf.0);
    s.delete_selected().unwrap();
    assert_eq!(s.task_for_key("k29"), None);
    s.rollback_to(1).unwrap();
    assert_eq!(s.task_for_key("k29"), Some(leaf));
    assert_eq!(snapshot(&s), before);
}

#[test]
fn a_paste_group_rolls_back_as_one() {
    let mut s = outline_session(30).0;
    let live = |s: &Session| s.schedule().tasks().iter().filter(|t| !t.deleted).count();
    let before = live(&s);
    s.select_task(0);
    s.insert_many(&["One", "Two", "Three"]).unwrap();
    assert_eq!(live(&s), before + 3);
    s.rollback_to(1).unwrap();
    assert_eq!((live(&s), s.history_len()), (before, (0, 0)));
    assert_eq!(sources(&mut s), ["paste", "rollback"]);
}

#[test]
fn a_rollback_to_an_acknowledged_commit_is_refused_and_changes_nothing() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    rename(&mut s, 2, "two");
    s.acknowledge(1);
    let before = (snapshot(&s), s.history_len());
    assert_eq!(s.rollback_to(1), Err(EditError::RollbackGone));
    assert_eq!((snapshot(&s), s.history_len()), before);
    assert_eq!(s.rollback_to(2), Ok(3));
}

#[test]
fn a_rollback_past_the_history_cap_is_refused() {
    let mut s = demo_session(20).0.with_history_cap(3);
    (1..=4).for_each(|task| rename(&mut s, task, "x"));
    let before = (snapshot(&s), s.history_len());
    assert_eq!(s.rollback_to(1), Err(EditError::RollbackGone));
    assert_eq!((snapshot(&s), s.history_len()), before);
    assert_eq!(s.rollback_to(2), Ok(5));
}

#[test]
fn a_rollback_during_a_bar_press_is_refused_busy() {
    let (mut s, task, _, (x, y)) = setup();
    let minutes = 960;
    s.edit(Edit::SetDuration {
        task: TaskId(task),
        minutes,
    })
    .unwrap();
    s.pointer_down(x, y, false);
    assert_eq!(s.rollback_to(1), Err(EditError::Busy));
}

#[test]
fn a_rollback_across_the_what_if_mark_ends_the_what_if() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    s.begin_whatif().unwrap();
    rename(&mut s, 2, "two");
    s.rollback_to(1).unwrap();
    assert!(s.whatif_summary().is_none());
}

#[test]
fn a_rollback_is_one_commit_and_cannot_be_rolled_back() {
    let mut s = demo_session(20).0;
    rename(&mut s, 1, "one");
    rename(&mut s, 2, "two");
    s.take_commits();
    assert_eq!(s.rollback_to(2), Ok(3));
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"][0]["source"], "rollback");
    assert_eq!(
        (out["commits"].as_array().unwrap().len(), &out["seq"]),
        (1, &Value::from(3))
    );
    rename(&mut s, 3, "three");
    let before = (snapshot(&s), s.history_len());
    assert_eq!(s.rollback_to(3), Err(EditError::RollbackGone));
    assert_eq!(s.rollback_to(2), Err(EditError::RollbackGone));
    assert_eq!((snapshot(&s), s.history_len()), before);
}

#[test]
fn a_step_the_engine_refuses_puts_back_what_ran() {
    let (mut s, meta) = outline_session(30);
    let leaf = leaf_of(&meta);
    rename(&mut s, leaf.0, "one");
    rename(&mut s, 29, "two");
    s.apply(Edit::Delete { task: leaf }).unwrap();
    let before = (snapshot(&s), s.history_len());
    assert!(s.rollback_to(1).is_err());
    assert_eq!((snapshot(&s), s.history_len()), before);
}
