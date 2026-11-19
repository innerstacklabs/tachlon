//! A host edit by key is one step of the person's undo history: it undoes in reverse order with their
//! edits, and a refused call leaves the history, the outbox and the document as they were.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::setup;
use crate::start_edit::session;
use app::{Session, document::Extras, document::write, edit::Edit, host::KeyTarget};
use engine::TaskId;
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

/// Three keyed tasks, `a` to `c`; a person renames the first; the one commit is drained.
fn renamed() -> Session {
    let mut s = session();
    for (n, key) in ["a", "b", "c"].into_iter().enumerate() {
        s.assign_key(KeyTarget::Task(TaskId(n as u32)), key)
            .unwrap();
    }
    s.edit(Edit::Rename {
        task: TaskId(0),
        name: "Mine".into(),
    })
    .unwrap();
    s.take_commits();
    s
}

/// The durations, the start limit of `b` and the name of `a`.
fn values(s: &Session) -> (Vec<i64>, Option<i64>, String) {
    let tasks = s.schedule().tasks();
    (
        tasks.iter().map(|t| t.duration).collect(),
        tasks[1].limit.snlt,
        s.meta()[0].name.clone(),
    )
}

const CALL: &str = r#"[{"key":"b","duration":120,"snlt":"2026-01-09"},{"key":"c","duration":180}]"#;

#[test]
fn a_host_edit_and_a_typed_edit_undo_in_reverse_order_and_redo_in_order() {
    let mut s = renamed();
    let (before, depth) = (values(&s), s.history_len().0);
    assert_eq!(s.host_edit(CALL).unwrap().tasks, 2);
    let after = values(&s);
    assert_eq!((&after.0[1..], after.1.is_some()), (&[120, 180][..], true));
    assert_eq!(s.history_len(), (depth + 1, 0));
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    let commits = out["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0]["source"], "host");
    assert_eq!(commits[0]["seq"], 2);
    assert!(s.undo().unwrap());
    assert_eq!(
        values(&s),
        before,
        "one Undo takes back both durations and the limit, not the name"
    );
    assert!(s.undo().unwrap());
    assert_eq!((values(&s).2 == "Mine", s.history_len()), (false, (0, 2)));
    assert!(s.redo().unwrap() && s.redo().unwrap());
    assert_eq!(values(&s), after);
}

#[test]
fn a_refused_call_leaves_the_history_the_outbox_and_the_document() {
    let mut s = renamed();
    let extras = Extras {
        name: "u",
        notes: &[],
        source: None,
    };
    let before = (write(&s, &extras), s.history_len(), s.revision());
    let cases = [
        (r#"{"key":"zz"}"#, "unknownKey"),
        (r#"{"key":"c","parent":"a"}"#, "notUpdatable"),
        (r#"{"key":"c","duration":"3"}"#, "badValue"),
        (r#"{"key":"c","duration":200000000}"#, "schedule"),
    ];
    for (bad, kind) in cases {
        let error = s
            .host_edit(&format!(r#"[{{"key":"b","duration":99}},{bad}]"#))
            .unwrap_err();
        assert_eq!((error.kind, error.record), (kind, Some(1)), "{bad}");
        assert_eq!(
            (write(&s, &extras), s.history_len(), s.revision()),
            before,
            "{bad}"
        );
        assert!(s.take_commits().starts_with(r#"{"commits":[],"#), "{bad}");
    }
}

#[test]
fn a_busy_press_answers_busy_and_a_read_only_chart_takes_the_call_without_asking() {
    let (mut s, _, _, (x, y)) = setup();
    s.pointer_down(x, y, false);
    assert_eq!(
        s.host_edit(r#"[{"key":"k0","duration":1}]"#)
            .unwrap_err()
            .kind,
        "busy"
    );
    let (mut s, asked) = (renamed(), Rc::new(Cell::new(0)));
    let seen = Rc::clone(&asked);
    s.set_before_edit(Some(Rc::new(move |_| {
        seen.set(seen.get() + 1);
        Ok(())
    })));
    s.set_read_only(true);
    s.host_edit(CALL).unwrap();
    assert_eq!((asked.get(), s.history_len().0), (0, 2));
}

#[test]
fn each_member_class_applies_undoes_and_redoes() {
    let extras = Extras {
        name: "u",
        notes: &[],
        source: None,
    };
    let members = r#""name":"New"|"duration":120|"start":"2026-01-12"|"snlt":"2026-01-15"|"fnet":"2026-01-16"|"fnlt":"2026-01-20""#;
    for member in members.split('|') {
        let mut s = renamed();
        let before = write(&s, &extras);
        s.host_edit(&format!(r#"[{{"key":"a",{member}}}]"#))
            .unwrap();
        let after = write(&s, &extras);
        assert_ne!(after, before, "{member}");
        assert!(s.undo().unwrap());
        assert_eq!(write(&s, &extras), before, "{member}");
        assert!(s.redo().unwrap());
        assert_eq!(write(&s, &extras), after, "{member}");
    }
}

#[test]
fn a_record_whose_undo_would_be_refused_is_refused_and_an_unchanged_call_changes_nothing() {
    let mut s = renamed();
    let edit = Edit::SetProgress {
        task: TaskId(1),
        actual_start: Some(0),
        actual_finish: None,
        remaining: None,
    };
    s.edit(edit).unwrap();
    let task = TaskId(2);
    let name = "x / y".into();
    s.apply(Edit::Rename { task, name }).unwrap();
    s.take_commits();
    let before = (s.history_len(), s.revision());
    for (record, kind) in [
        (r#"{"key":"b","duration":1}"#, "hasActuals"),
        (r#"{"key":"a","progress":50}"#, "hasActuals"),
        (r#"{"key":"c","name":"N"}"#, "notUndoable"),
    ] {
        let error = s.host_edit(&format!("[{record}]")).unwrap_err();
        assert_eq!((error.kind, error.record), (kind, Some(0)), "{record}");
    }
    let duration = s.schedule().tasks()[2].duration;
    let same = format!(r#"[{{"key":"c","duration":{duration}}}]"#);
    assert_eq!(s.host_edit(&same).unwrap().solves, 0);
    assert_eq!((s.history_len(), s.revision()), before);
    assert!(s.take_commits().starts_with(r#"{"commits":[],"#));
}
