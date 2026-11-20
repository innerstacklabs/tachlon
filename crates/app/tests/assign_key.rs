//! A host hands back the key it gave a task or link the chart made (gantt#2082): not an edit, kept by
//! the document, refused by name, and valid for a slot an undo has tombstoned.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::session_of;
use app::{
    Session,
    document::{Extras, read, write},
    edit::Edit,
    host::KeyTarget,
    load_bytes,
};
use engine::{DepId, LinkType, TaskId};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

fn saved(session: &Session) -> String {
    let extras = Extras {
        name: "keys",
        notes: &[],
        source: None,
    };
    write(session, &extras)
}

/// A session of an XER with six activities, the first three leaf tasks, and the first task keyed `held` when
/// the session is the keyed one: a save and a read, as a host loads it.
fn opened(keyed: bool) -> (Session, Vec<TaskId>) {
    let xer = synth_xer(&Spec::with_activities(7, 6));
    let mut loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).unwrap();
    let leaves: Vec<TaskId> = loaded
        .outline
        .preorder()
        .iter()
        .copied()
        .filter(|&t| !loaded.outline.is_summary(t))
        .take(3)
        .collect();
    if !keyed {
        return (session_of(loaded), leaves);
    }
    loaded.meta[leaves[0].0 as usize].key = Some("held".into());
    let text = saved(&session_of(loaded));
    let document = read(&text, &|| 0.0).unwrap();
    let leaves = document
        .loaded
        .outline
        .preorder()
        .iter()
        .copied()
        .filter(|&t| !document.loaded.outline.is_summary(t))
        .take(3)
        .collect();
    (
        session_of(document.loaded).with_host_keys(document.keys),
        leaves,
    )
}

fn insert(session: &mut Session, after: TaskId) -> TaskId {
    session.select(after.0);
    session.insert_below_selection().unwrap()
}

fn assign(session: &mut Session, task: TaskId, key: &str) -> Result<(), &'static str> {
    session
        .assign_key(KeyTarget::Task(task), key)
        .map_err(|e| e.kind())
}

fn link(session: &mut Session, pred: TaskId, succ: TaskId) -> DepId {
    let edit = Edit::AddLink {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    };
    session.edit(edit).unwrap();
    DepId(u32::try_from(session.schedule().dependencies().len() - 1).unwrap())
}

#[test]
fn a_key_is_no_edit_and_the_document_names_it() {
    for keyed in [false, true] {
        let (mut session, leaves) = opened(keyed);
        let task = insert(&mut session, leaves[1]);
        let (history, revision) = (session.history_len(), session.revision());
        session.take_commits();
        assert_eq!(assign(&mut session, task, "fresh"), Ok(()));
        assert!(session.take_commits().starts_with(r#"{"commits":[]"#));
        assert_eq!(
            (session.history_len(), session.revision()),
            (history, revision)
        );
        assert!(saved(&session).contains(r#""key":"fresh""#));
        assert_eq!(
            assign(&mut session, task, "fresh"),
            Ok(()),
            "the same key twice"
        );
        assert_eq!(session.host_keys().task("fresh"), Some(task));
    }
}

#[test]
fn a_refusal_changes_nothing_and_names_its_kind() {
    let (mut session, leaves) = opened(true);
    let task = insert(&mut session, leaves[1]);
    let before = saved(&session);
    assert_eq!(assign(&mut session, task, "held"), Err("keyTaken"));
    assert_eq!(assign(&mut session, task, ""), Err("emptyKey"));
    assert_eq!(assign(&mut session, TaskId(9999), "x"), Err("unknownTask"));
    assert_eq!(assign(&mut session, leaves[0], "other"), Err("hasKey"));
    let missing = session.assign_key(KeyTarget::Link(DepId(9999)), "x");
    assert_eq!(missing.unwrap_err().kind(), "unknownLink");
    assert_eq!(saved(&session), before);
}

#[test]
fn a_deleted_task_keeps_its_key_taken_and_a_tombstone_takes_one() {
    let (mut session, leaves) = opened(false);
    let first = insert(&mut session, leaves[1]);
    assert_eq!(assign(&mut session, first, "gone"), Ok(()));
    session.edit(Edit::Delete { task: first }).unwrap();
    let second = insert(&mut session, leaves[1]);
    assert_eq!(assign(&mut session, second, "gone"), Err("keyTaken"));
    let third = insert(&mut session, leaves[1]);
    assert!(session.undo().unwrap(), "the insert of the third is undone");
    assert_eq!(
        assign(&mut session, third, "late"),
        Ok(()),
        "a tombstoned slot takes a key"
    );
    assert!(session.redo().unwrap());
    assert_eq!(session.host_keys().task("late"), Some(third));
    assert!(saved(&session).contains(r#""key":"late""#));
}

#[test]
fn a_drawn_link_gets_a_key_and_a_second_link_cannot_have_it() {
    let (mut session, leaves) = opened(false);
    let (a, b, c) = (
        insert(&mut session, leaves[2]),
        insert(&mut session, leaves[2]),
        insert(&mut session, leaves[2]),
    );
    let (one, two) = (link(&mut session, a, b), link(&mut session, b, c));
    let (history, revision) = (session.history_len(), session.revision());
    session.assign_key(KeyTarget::Link(one), "l-1").unwrap();
    assert_eq!(
        (session.history_len(), session.revision()),
        (history, revision)
    );
    let taken = session.assign_key(KeyTarget::Link(two), "l-1");
    assert_eq!(taken.unwrap_err().kind(), "keyTaken");
    assert_eq!(
        session.host_keys().link_key(two),
        None,
        "a refusal leaves the link keyless"
    );
    assert!(saved(&session).contains(r#""key":"l-1""#));
    assert_eq!(session.host_keys().link("l-1"), Some(one));
}

fn rekey(session: &mut Session, target: KeyTarget, key: &str) -> Result<(), &'static str> {
    session.rekey(target, key).map_err(|e| e.kind())
}

#[test]
fn a_rekey_moves_the_key_of_a_task_and_frees_the_old_one() {
    let (mut session, leaves) = opened(true);
    let task = KeyTarget::Task(leaves[0]);
    let (history, revision) = (session.history_len(), session.revision());
    assert_eq!(rekey(&mut session, task, "new"), Ok(()));
    assert_eq!(session.host_keys().task("new"), Some(leaves[0]));
    assert_eq!(session.host_keys().task("held"), None);
    let text = saved(&session);
    assert!(text.contains(r#""key":"new""#) && !text.contains(r#""key":"held""#));
    assert_eq!(
        (session.history_len(), session.revision()),
        (history, revision)
    );
    assert_eq!(assign(&mut session, leaves[1], "held"), Ok(()));
}

#[test]
fn a_rekey_refuses_a_key_another_task_holds_and_changes_nothing() {
    let (mut session, leaves) = opened(true);
    let (live, gone) = (
        insert(&mut session, leaves[1]),
        insert(&mut session, leaves[1]),
    );
    assert_eq!(assign(&mut session, live, "live"), Ok(()));
    assert_eq!(assign(&mut session, gone, "gone"), Ok(()));
    session.edit(Edit::Delete { task: gone }).unwrap();
    let (before, task) = (saved(&session), KeyTarget::Task(leaves[0]));
    assert_eq!(rekey(&mut session, task, "live"), Err("keyTaken"));
    assert_eq!(rekey(&mut session, task, "gone"), Err("keyTaken"));
    assert_eq!(saved(&session), before);
    assert_eq!(rekey(&mut session, task, "held"), Ok(()), "the same key");
    assert_eq!(assign(&mut session, leaves[0], "other"), Err("hasKey"));
}

#[test]
fn a_rekey_moves_the_key_of_a_link() {
    let (mut session, leaves) = opened(false);
    let (a, b, c) = (
        insert(&mut session, leaves[2]),
        insert(&mut session, leaves[2]),
        insert(&mut session, leaves[2]),
    );
    let (one, two) = (link(&mut session, a, b), link(&mut session, b, c));
    session.assign_key(KeyTarget::Link(one), "l-1").unwrap();
    let (history, revision) = (session.history_len(), session.revision());
    assert_eq!(rekey(&mut session, KeyTarget::Link(one), "l-2"), Ok(()));
    let keys = session.host_keys();
    assert_eq!(
        (keys.link_key(one), keys.link("l-2"), keys.link("l-1")),
        (Some("l-2"), Some(one), None)
    );
    assert_eq!(
        (session.history_len(), session.revision()),
        (history, revision)
    );
    session.assign_key(KeyTarget::Link(two), "l-1").unwrap();
    assert_eq!(
        rekey(&mut session, KeyTarget::Link(two), "l-2"),
        Err("keyTaken")
    );
}
