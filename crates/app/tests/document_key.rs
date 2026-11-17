//! A task keeps the opaque key its host gave it (gantt#2082): the document writes it only when
//! set and reads it back, rename, delete and undo leave it alone, and the XER and MS Project
//! exports name it as left out.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use super::document_round_trip::{refusal, session_of};
use app::{
    Session,
    document::{DocumentError, Extras, read, write},
    edit::Edit,
    export::{export_mspdi, export_xer},
    load_bytes,
};
use engine::{CalendarId, DepId, LinkType, TaskId};
use import::ProjectSelection;
use xer::synth::{Spec, write as synth_xer};

const KEYS: [&str; 3] = [
    "6f1c2d5e-0b7a-4c1e-9a53-2d8f7e4b1a90",
    "b0d4a8c2-71e3-4f65-8a1d-3c9e5f20b7d4",
    "e2a9c7f1-5d38-4b06-9c4e-8f1a6d3b2e75",
];

fn text_of(session: &Session) -> String {
    let extras = Extras {
        name: "keys",
        notes: &[],
        source: None,
    };
    write(session, &extras)
}

/// A session of an XER with six activities, and the ids of the first three leaf tasks, which hold
/// `KEYS` when `keyed`.
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
    assert_eq!(leaves.len(), 3, "three leaf tasks");
    if keyed {
        for (task, key) in leaves.iter().zip(KEYS) {
            loaded.meta[task.0 as usize].key = Some(key.to_string());
        }
    }
    (session_of(loaded), leaves)
}

/// The keys of the tasks a document holds, in document order.
fn keys_in(text: &str) -> Vec<String> {
    let loaded = read(text, &|| 0.0).unwrap().loaded;
    loaded.meta.iter().filter_map(|m| m.key.clone()).collect()
}

#[test]
fn keys_come_back_after_a_rename_a_delete_an_undo_and_a_save() {
    let (mut session, leaves) = opened(true);
    let saved = text_of(&session);
    assert_eq!(keys_in(&saved), KEYS);
    session
        .edit(Edit::Rename {
            task: leaves[0],
            name: "Renamed".into(),
        })
        .unwrap();
    session.edit(Edit::Delete { task: leaves[1] }).unwrap();
    // The renamed task still has its key; the deleted one is not in the document at all.
    let edited = text_of(&session);
    assert_eq!(keys_in(&edited), [KEYS[0], KEYS[2]]);
    assert!(edited.contains(&format!(r#""name":"Renamed","key":"{}""#, KEYS[0])));
    assert!(session.undo().unwrap(), "the delete is undone");
    let undone = text_of(&session);
    assert_eq!(keys_in(&undone), KEYS);
    // A reopened document writes the same text, and a document with no keys writes none.
    let loaded = read(undone.as_str(), &|| 0.0).unwrap().loaded;
    assert_eq!(text_of(&session_of(loaded)), undone);
    assert!(!text_of(&opened(false).0).contains("\"key\""));
}

#[test]
fn a_key_that_is_not_text_is_refused_by_name() {
    let (session, _) = opened(true);
    let text = text_of(&session);
    let bad = text.replacen(&format!("\"{}\"", KEYS[0]), "5", 1);
    let error = refusal(&bad).to_string();
    assert!(error.contains("key: expected a string"), "{error}");
}

#[test]
fn both_exports_name_the_keys_as_left_out() {
    let (session, _) = opened(true);
    let extras = Extras {
        name: "keys",
        notes: &[],
        source: None,
    };
    let note = "host keys on 3 task(s) are not written";
    let xer = export_xer(&session, &extras).unwrap();
    let mspdi = export_mspdi(&session, &extras).unwrap();
    for left_out in [&xer.left_out, &mspdi.left_out] {
        assert_eq!(
            left_out.iter().filter(|l| *l == note).count(),
            1,
            "{left_out:?}"
        );
    }
    let (plain, _) = opened(false);
    for exported in [
        export_xer(&plain, &extras).unwrap(),
        export_mspdi(&plain, &extras).unwrap(),
    ] {
        assert!(exported.left_out.iter().all(|l| !l.contains("host keys")));
    }
}

const LINKS: [&str; 2] = [
    "3a7e90d1-c4b2-4e68-b1f5-6d02a8c9e347",
    "9c41f6b8-2e0d-4a73-85b9-e1d7c3a60f52",
];
const CALENDAR: &str = "d85b2e60-4f17-4c9a-a3e8-7b0c19f4d263";

/// The document of `opened(true)` with `LINKS` on its first two links and `CALENDAR` on its last calendar.
fn keyed_document() -> String {
    let mut text = text_of(&opened(true).0);
    for key in LINKS {
        let keyed = format!("\"lag_calendar\":null,\"key\":\"{key}\"}}");
        text = text.replacen("\"lag_calendar\":null}", &keyed, 1);
    }
    let keyed = format!(",\"key\":\"{CALENDAR}\"}}],\"tasks\":[");
    text.replacen("}],\"tasks\":[", &keyed, 1)
}

fn reopened(text: &str) -> Session {
    let document = read(text, &|| 0.0).unwrap();
    session_of(document.loaded).with_host_keys(document.keys)
}

/// The names of the tasks the link `key` joins.
fn ends(session: &Session, key: &str) -> (String, String) {
    let dep = session.host_keys().link(key).unwrap();
    let link = session.schedule().dependencies()[dep.0 as usize];
    let name = |task: TaskId| session.meta()[task.0 as usize].name.clone();
    (name(link.pred), name(link.succ))
}

#[test]
fn a_link_and_a_calendar_keep_their_key_through_save_and_reopen() {
    let text = keyed_document();
    let session = reopened(&text);
    let keys = session.host_keys();
    assert_eq!(
        (keys.link(LINKS[0]), keys.link(LINKS[1])),
        (Some(DepId(0)), Some(DepId(1)))
    );
    let last = u32::try_from(session.schedule().calendars().len() - 1).unwrap();
    assert_eq!(keys.calendar(CALENDAR), Some(CalendarId(last)));
    let task = keys.task(KEYS[1]).unwrap();
    assert_eq!(
        session.meta()[task.0 as usize].key.as_deref(),
        Some(KEYS[1])
    );
    assert_eq!(text_of(&session), text, "the same bytes come back");
}

#[test]
fn a_link_key_stays_on_its_link_through_a_remove_an_undo_a_new_link_and_a_deleted_task() {
    let text = keyed_document();
    let mut session = reopened(&text);
    let before = [ends(&session, LINKS[0]), ends(&session, LINKS[1])];
    session.edit(Edit::RemoveLink { dep: DepId(0) }).unwrap();
    let removed = text_of(&session);
    assert!(!removed.contains(LINKS[0]) && removed.contains(LINKS[1]));
    assert!(session.undo().unwrap());
    assert_eq!(text_of(&session), text);
    // A link drawn in the chart has no key, and the others keep theirs.
    let outline = session.outline().unwrap();
    let leaves: Vec<TaskId> = (outline.preorder().iter().copied())
        .filter(|&t| !outline.is_summary(t))
        .collect();
    let new = leaves
        .iter()
        .flat_map(|&p| leaves.iter().map(move |&s| (p, s)))
        .find_map(|(pred, succ)| {
            let edit = Edit::AddLink {
                pred,
                succ,
                kind: LinkType::FS,
                lag: 0,
            };
            session.edit(edit).ok()
        });
    assert!(new.is_some(), "a link can be added");
    let added = DepId(u32::try_from(session.schedule().dependencies().len() - 1).unwrap());
    assert_eq!(session.host_keys().link_key(added), None);
    let saved = reopened(&text_of(&session));
    assert_eq!([ends(&saved, LINKS[0]), ends(&saved, LINKS[1])], before);
    // A deleted task takes its link out of the document, and its restore brings the link back with its key.
    let (pred, _) = {
        let link = session.schedule().dependencies()[0];
        (link.pred, link.succ)
    };
    session.edit(Edit::Delete { task: pred }).unwrap();
    assert!(!text_of(&session).contains(LINKS[0]));
    assert_eq!(
        session.host_keys().link(LINKS[0]),
        Some(DepId(0)),
        "the key stays taken"
    );
    assert!(session.undo().unwrap(), "the delete is undone");
    let saved = reopened(&text_of(&session));
    assert_eq!([ends(&saved, LINKS[0]), ends(&saved, LINKS[1])], before);
}

#[test]
fn a_key_that_is_not_text_is_empty_or_repeated_is_refused_by_name() {
    let text = keyed_document();
    let not_text = text.replacen(&format!("\"key\":\"{}\"", LINKS[0]), "\"key\":5", 1);
    let error = refusal(&not_text).to_string();
    assert!(error.contains("links[0].key: expected a string"), "{error}");
    let empty = text.replacen(&format!("\"key\":\"{CALENDAR}\""), "\"key\":\"\"", 1);
    assert!(refusal(&empty).to_string().contains("calendars["), "empty");
    let repeated = text.replacen(LINKS[1], LINKS[0], 1);
    assert_eq!(
        refusal(&repeated),
        DocumentError::DuplicateKey {
            key: LINKS[0].into(),
            first: "links[0]".into(),
            again: "links[1]".into()
        }
    );
    let twice = refusal(&text.replacen(KEYS[1], KEYS[0], 1));
    assert!(
        matches!(&twice, DocumentError::DuplicateKey { key, first, again }
        if key == KEYS[0] && first != again && first.starts_with("tasks[")),
        "{twice}"
    );
    // One text may be the key of a task, a link and a calendar.
    let shared = text
        .replacen(LINKS[0], KEYS[0], 1)
        .replacen(CALENDAR, KEYS[0], 1);
    assert!(read(&shared, &|| 0.0).is_ok());
}

#[test]
fn both_exports_name_link_and_calendar_keys_as_left_out() {
    let session = reopened(&keyed_document());
    let extras = Extras {
        name: "keys",
        notes: &[],
        source: None,
    };
    let note = "host keys on 3 task(s), 2 link(s), 1 calendar(s) are not written";
    for exported in [
        export_xer(&session, &extras).unwrap(),
        export_mspdi(&session, &extras).unwrap(),
    ] {
        assert_eq!(
            exported.left_out.iter().filter(|l| *l == note).count(),
            1,
            "{:?}",
            exported.left_out
        );
    }
}

#[test]
fn task_for_key_answers_the_task_that_holds_a_key_and_nothing_for_an_unknown_or_deleted_key() {
    let mut session = reopened(&text_of(&opened(true).0));
    let task = session.host_keys().task(KEYS[1]).unwrap();
    assert_eq!(session.task_for_key(KEYS[1]), Some(task));
    assert_eq!(session.task_for_key("no-such-key"), None);
    session.edit(Edit::Delete { task }).unwrap();
    assert_eq!(
        session.task_for_key(KEYS[1]),
        None,
        "a deleted task has no live slot"
    );
    assert!(session.undo().unwrap());
    assert_eq!(
        session.task_for_key(KEYS[1]),
        Some(task),
        "an undo brings it back"
    );
}
