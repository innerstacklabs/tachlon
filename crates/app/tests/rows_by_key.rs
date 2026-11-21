//! A host adds and removes a task by key (tl#2087): no undo step and no commit, and the person's undo and redo
//! steps that named the row are forgotten, so their Undo reaches the steps before them.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::on;
use crate::reparent::build;
use app::edit::{Edit, EditError};
use app::{Place, Session, Siblings, history::Source, host::KeyTarget};
use engine::{DepId, Limit, LinkType, Pin, TaskId};
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

/// A person's insert of a row `Y` (slot 6) after C, on `keyed()` with A renamed first.
fn inserted_after_c() -> Session {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    let place = Place {
        parent: None,
        after: Some(C),
    };
    s.edit(Edit::Insert {
        place,
        name: "Y".into(),
    })
    .unwrap();
    s
}

#[test]
fn a_row_deleted_after_the_removed_one_loses_its_older_steps_too() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    rename(&mut s, D, "d1");
    s.edit(Edit::Delete { task: D }).unwrap();
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, A), s.history_len()), ("A", (0, 1)));
}

#[test]
fn a_row_inserted_and_deleted_after_the_removed_one_loses_its_insert_step() {
    let mut s = inserted_after_c();
    s.edit(Edit::Delete { task: TaskId(6) }).unwrap();
    s.remove_task_by_key("t3").unwrap();
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, A), s.history_len()), ("A", (0, 1)));
}

#[test]
fn an_undone_insert_after_the_removed_row_loses_its_redo_steps() {
    let mut s = inserted_after_c();
    rename(&mut s, TaskId(6), "y1");
    assert!(s.undo().unwrap() && s.undo().unwrap());
    s.remove_task_by_key("t3").unwrap();
    assert_eq!((s.redo(), s.history_len()), (Ok(false), (1, 0)));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, A), "A");
}

fn add(s: &mut Session, json: &str, parent: Option<&str>, after: Option<&str>) -> TaskId {
    s.add_task_by_key(json, parent, after).unwrap()
}

/// The refusal's kind and member.
fn refused(
    s: &mut Session,
    json: &str,
    parent: Option<&str>,
    after: Option<&str>,
) -> (&'static str, Option<String>) {
    let error = s.add_task_by_key(json, parent, after).unwrap_err();
    (error.kind, error.member)
}

#[test]
fn adding_by_key_leaves_the_undo_list_the_revision_the_outbox_and_the_selection() {
    let mut s = keyed();
    rename(&mut s, A, "Mine");
    s.take_commits();
    assert!(s.select_task(E.0));
    let (history, revision) = (s.history_len(), s.revision());
    let task = add(&mut s, r#"{"key":"n1"}"#, None, None);
    assert_eq!(s.task_for_key("n1"), Some(task));
    assert_eq!((s.history_len(), s.revision()), (history, revision));
    assert_eq!(s.selection().map(|selected| selected.task), Some(E.0));
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 0, "no commit");
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, A), s.task_for_key("n1")), ("A", Some(task)));
}

#[test]
fn adding_by_key_places_the_row_by_parent_and_after_keys_and_sets_the_record() {
    let mut s = keyed();
    let set = r#"{"key":"n1","name":"A2","duration":120}"#;
    let a2 = add(&mut s, set, Some("t0"), Some("t1"));
    assert_eq!(
        (name(&s, a2), s.schedule().tasks()[a2.0 as usize].duration),
        ("A2", 120)
    );
    let last = add(&mut s, r#"{"key":"n2"}"#, Some("t0"), None);
    let after_b = add(&mut s, r#"{"key":"n3"}"#, None, Some("t2"));
    let root = add(&mut s, r#"{"key":"n4"}"#, None, None);
    let outline = s.outline().unwrap();
    assert_eq!(outline.children(P), [A, a2, B, after_b, last]);
    assert_eq!(outline.roots(), [P, C, D, E, root]);
}

#[test]
fn adding_by_key_while_a_filter_or_grouping_is_on_is_taken_and_a_typed_insert_is_still_refused() {
    let mut s = keyed();
    assert!(s.set_critical_only(true));
    add(&mut s, r#"{"key":"n1"}"#, None, None);
    assert_eq!(s.insert_below_selection(), Err(EditError::FilterActive));
    assert!(s.set_critical_only(false) && s.set_group_by(Some("Zone")));
    add(&mut s, r#"{"key":"n2"}"#, None, None);
    assert_eq!(s.insert_below_selection(), Err(EditError::InsertGrouped));
    assert_eq!(s.task_count(), 8);
}

#[test]
fn a_duplicate_unknown_or_bad_record_is_refused_by_name_and_changes_nothing() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    let held = |s: &Session| {
        (
            s.history_len(),
            s.revision(),
            s.task_count(),
            s.solution().dates[..6].to_vec(),
        )
    };
    let before = held(&s);
    let named = |kind, member: Option<&str>| (kind, member.map(str::to_owned));
    let cases = [
        (
            r#"{"key":"t3"}"#,
            None,
            None,
            named("duplicateKey", Some("key")),
        ),
        (r#"{"name":"x"}"#, None, None, named("noKey", Some("key"))),
        ("[1]", None, None, named("badJson", None)),
        (
            r#"{"key":"n1"}"#,
            Some("nobody"),
            None,
            named("unknownKey", Some("parent")),
        ),
        (
            r#"{"key":"n1"}"#,
            None,
            Some("nobody"),
            named("unknownKey", Some("after")),
        ),
        (
            r#"{"key":"n1","duration":-1}"#,
            None,
            None,
            named("badValue", Some("duration")),
        ),
        (
            r#"{"key":"n1","alap":true}"#,
            None,
            None,
            named("notUpdatable", Some("alap")),
        ),
    ];
    for (json, parent, after, refusal) in cases {
        assert_eq!(refused(&mut s, json, parent, after), refusal, "{json}");
        assert_eq!(held(&s), before, "{json} changes nothing");
    }
    assert_eq!(
        s.host_keys().task("n1"),
        None,
        "a late refusal frees the key"
    );
    let task = add(&mut s, r#"{"key":"n1"}"#, None, None);
    assert_eq!(s.task_for_key("n1"), Some(task));
}

#[test]
fn a_busy_press_refuses_an_add_as_busy() {
    let mut s = keyed();
    let (x, y) = on(&s, C.0);
    s.pointer_down(x, y, false);
    assert_eq!(
        refused(&mut s, r#"{"key":"t3"}"#, None, None).0,
        "busy",
        "before any other refusal"
    );
    assert_eq!(refused(&mut s, r#"{"key":"n1"}"#, None, None).0, "busy");
    assert_eq!((s.host_keys().task("n1"), s.task_count()), (None, 6));
}

#[test]
fn task_members_read_back_into_update_tasks_change_nothing() {
    let mut s = keyed();
    s.edit(Edit::Move {
        task: C,
        snet: Some(600),
    })
    .unwrap();
    let limit = Limit {
        snlt: Some(3_000),
        fnet: Some(2_000),
        fnlt: Some(9_000),
        ..Limit::default()
    };
    s.edit(Edit::SetLimit { task: C, limit }).unwrap();
    let limit = Limit {
        pin: Some(Pin::Finish(4_000)),
        ..Limit::default()
    };
    s.edit(Edit::SetLimit { task: E, limit }).unwrap();
    let held = |s: &Session| {
        (
            s.schedule().tasks().to_vec(),
            s.meta()[3].name.clone(),
            s.history_len(),
            s.solution().dates.clone(),
        )
    };
    let before = held(&s);
    for key in ["t3", "t5"] {
        let record = s.task_members(key).unwrap();
        let read: Value = serde_json::from_str(&record).unwrap();
        assert_eq!(
            (read["key"].as_str(), read["name"].as_str()),
            (Some(key), Some(name(&s, s.task_for_key(key).unwrap())))
        );
        assert!(
            ["duration", "snet", "snlt", "fnet", "fnlt", "mso", "mfo"]
                .iter()
                .all(|m| read.get(m).is_some()),
            "{record}"
        );
        s.update_tasks(&format!("[{record}]")).unwrap();
        assert_eq!(held(&s), before, "{record}");
    }
    let read: Value = serde_json::from_str(&s.task_members("t5").unwrap()).unwrap();
    assert!(read["mfo"].is_string() && read["mso"].is_null());
    s.remove_task_by_key("t4").unwrap();
    assert_eq!(s.task_members("t4").unwrap_err().kind, "unknownKey");
}

#[test]
fn an_add_that_restores_a_deleted_row_forgets_the_persons_restore_step() {
    let mut s = keyed();
    rename(&mut s, A, "a1");
    rename(&mut s, E, "e1");
    s.edit(Edit::Delete { task: E }).unwrap();
    let task = add(&mut s, r#"{"key":"t5","name":"E2"}"#, None, Some("t3"));
    assert_eq!(
        (task, s.task_for_key("t5"), name(&s, E)),
        (E, Some(E), "E2")
    );
    assert_eq!(s.outline().unwrap().roots(), [P, C, E, D]);
    assert_eq!(
        s.history_len(),
        (1, 0),
        "the rename and the delete of E are forgotten"
    );
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(
        (name(&s, A), s.task_for_key("t5"), name(&s, E)),
        ("A", Some(E), "E2")
    );
}

#[test]
fn a_key_freed_by_a_remove_can_be_added_again() {
    let mut s = keyed();
    s.remove_task_by_key("t4").unwrap();
    let task = add(&mut s, r#"{"key":"t4","name":"D2"}"#, None, None);
    assert_ne!(task, D);
    assert_eq!((s.task_for_key("t4"), name(&s, task)), (Some(task), "D2"));
    assert!(s.schedule().is_deleted(D));
}
