//! A host removes a task by key (tl#2087): no undo step and no commit, and the person's undo and redo steps
//! that named the row are forgotten, so their Undo reaches the steps before them.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::on;
use crate::reparent::build;
use app::{Place, Session, Siblings, edit::Edit, history::Source, host::KeyTarget};
use engine::{DepId, LinkType, TaskId};
use serde_json::Value;

const P: TaskId = TaskId(0);
const A: TaskId = TaskId(1);
const B: TaskId = TaskId(2);
const C: TaskId = TaskId(3);
const D: TaskId = TaskId(4);
const E: TaskId = TaskId(5);

/// P / A, B under it / C, D, E: links B to C (0), C to D (1) and B to D (2), and each task keyed `t<slot>`.
fn keyed() -> Session {
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
        let key = format!("t{n}");
        s.assign_key(KeyTarget::Task(TaskId(n)), &key).unwrap();
    }
    s
}

fn rename(s: &mut Session, task: TaskId, name: &str) {
    let name = name.into();
    s.edit(Edit::Rename { task, name }).unwrap();
}

fn name(s: &Session, task: TaskId) -> &str {
    &s.meta()[task.0 as usize].name
}

#[test]
fn a_busy_press_refuses_a_remove_as_busy() {
    let mut s = keyed();
    let (x, y) = on(&s, C.0);
    s.pointer_down(x, y, false);
    assert_eq!(s.remove_task_by_key("t3").unwrap_err().kind, "busy");
    assert_eq!(s.task_for_key("t3"), Some(C));
}

#[test]
fn removing_by_key_leaves_the_undo_list_and_a_selected_other_row() {
    let mut s = keyed();
    rename(&mut s, A, "Mine");
    s.take_commits();
    assert!(s.select_task(E.0));
    let (history, revision) = (s.history_len(), s.revision());
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.task_for_key("t3"), None);
    assert_eq!(s.host_keys().task("t3"), None, "the key is free");
    assert_eq!((s.history_len(), s.revision()), (history, revision));
    assert_eq!(s.selection().map(|selected| selected.task), Some(E.0));
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 0, "no commit");
}

#[test]
fn a_removed_tasks_links_go_with_it_and_the_rest_still_solve() {
    let (mut s, mut person) = (keyed(), keyed());
    person.edit(Edit::Delete { task: C }).unwrap();
    s.remove_task_by_key("t3").unwrap();
    let links = s.schedule().live_dependencies();
    assert!(
        links
            .map(|(_, d)| [d.pred, d.succ])
            .all(|ends| !ends.contains(&C))
    );
    assert_eq!(s.schedule().live_dependencies().count(), 1);
    assert_eq!(s.solution().dates, person.solution().dates);
}

#[test]
fn removing_a_row_with_children_is_refused_and_children_first_is_taken() {
    let mut s = keyed();
    assert_eq!(s.remove_task_by_key("t0").unwrap_err().kind, "hasChildren");
    assert_eq!(s.task_for_key("t0"), Some(P));
    for key in ["t1", "t2", "t0"] {
        s.remove_task_by_key(key).unwrap();
    }
    assert_eq!(s.task_for_key("t0"), None);
    assert_eq!(
        s.remove_task_by_key("nobody").unwrap_err().kind,
        "unknownKey"
    );
}

#[test]
fn undo_after_a_remove_skips_the_steps_that_named_the_row_and_reaches_the_older_ones() {
    let mut s = keyed();
    rename(&mut s, A, "Older");
    rename(&mut s, C, "Gone");
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, A), s.history_len()), ("A", (0, 1)));
}

#[test]
fn undo_and_redo_after_a_remove_skip_the_link_steps_of_the_row() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    let (kind, lag) = (LinkType::FS, 0);
    s.edit(Edit::AddLink {
        pred: C,
        succ: E,
        kind,
        lag,
    })
    .unwrap();
    s.edit(Edit::RemoveLink { dep: DepId(1) }).unwrap();
    rename(&mut s, B, "b1");
    assert!(s.undo().unwrap() && s.undo().unwrap());
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.history_len(), (1, 1));
    assert_eq!(s.redo(), Ok(true));
    assert_eq!(
        (s.undo(), s.undo(), s.undo()),
        (Ok(true), Ok(true), Ok(false))
    );
    assert_eq!((name(&s, A), name(&s, B)), ("A", "B"));
}

#[test]
fn a_counted_reparent_is_forgotten_with_the_row_and_one_without_a_count_is_kept() {
    let mut s = keyed();
    let out = Place {
        parent: None,
        after: Some(P),
    };
    let siblings = Siblings::Adopt(1);
    s.edit(Edit::Reparent {
        task: A,
        place: out,
        siblings,
    })
    .unwrap();
    let under = Place {
        parent: Some(P),
        after: None,
    };
    let siblings = Siblings::Keep;
    s.edit(Edit::Reparent {
        task: E,
        place: under,
        siblings,
    })
    .unwrap();
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.history_len(), (1, 0), "only the plain reparent is kept");
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.outline().unwrap().parent(E), None);
}

#[test]
fn a_forget_below_the_whatif_mark_keeps_discard_undoing_exactly_the_whatif_edits() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    rename(&mut s, C, "c1");
    s.begin_whatif().unwrap();
    rename(&mut s, B, "b1");
    s.remove_task_by_key("t3").unwrap();
    s.discard_whatif().unwrap();
    assert_eq!((name(&s, A), name(&s, B)), ("a1", "B"));
    assert_eq!(s.history_len().0, 1);
}

#[test]
fn a_remove_that_forgot_steps_makes_rollback_refuse() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    rename(&mut s, C, "c1");
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.rollback_to(2).unwrap_err().kind(), "rollbackGone");
    assert_eq!(
        name(&s, A),
        "a1",
        "the older step is not walked in its place"
    );
}

#[test]
fn removing_a_person_deleted_row_changes_nothing_and_keeps_its_undo() {
    let mut s = keyed();
    s.edit(Edit::Delete { task: E }).unwrap();
    let history = s.history_len();
    assert_eq!(s.remove_task_by_key("t5"), Ok(()));
    assert_eq!(
        (s.history_len(), s.host_keys().task("t5")),
        (history, Some(E))
    );
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.task_for_key("t5"), Some(E));
}

#[test]
fn a_group_that_touched_the_row_is_forgotten_whole() {
    let mut s = keyed();
    rename(&mut s, D, "d1");
    let rename_to = |task, name: &str| Edit::Rename {
        task,
        name: name.into(),
    };
    let both = vec![rename_to(A, "a1"), rename_to(C, "c1")];
    s.edit_group(both, Source::Typed).unwrap();
    s.remove_task_by_key("t3").unwrap();
    assert_eq!((s.history_len(), name(&s, A)), ((1, 0), "a1"));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, D), "D");
}
