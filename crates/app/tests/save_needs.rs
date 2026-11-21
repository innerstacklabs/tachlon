#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::history::snapshot;
use crate::link_gesture::{link, on, session};
use crate::read_only::longer;
use app::{Session, WhatIfError, edit::Edit, edit::EditError, history::Source, host::Planned};
use engine::{DepId, LinkType, TaskId};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

const REFUSED: &str = "This page cannot save new links: it gave no createLink.";

fn spy(s: &mut Session, missing: &'static str) -> Rc<RefCell<Vec<Planned>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    s.set_before_edit(Some(Rc::new(move |p: &Planned| {
        log.borrow_mut().push(p.clone());
        (!p.needs.contains(&missing))
            .then_some(())
            .ok_or(EditError::Refused(REFUSED.into()))
    })));
    seen
}

fn links() -> Vec<Edit> {
    [0, 1]
        .map(|pred| Edit::AddLink {
            pred: TaskId(pred),
            succ: TaskId(2),
            kind: LinkType::FS,
            lag: 0,
        })
        .to_vec()
}

fn commits(s: &mut Session) -> Value {
    serde_json::from_str::<Value>(&s.take_commits()).unwrap()["commits"].clone()
}

type Act = fn(&mut Session);

fn actions() -> Vec<(&'static str, Act)> {
    vec![
        ("plain edit", |s| s.edit(longer(300)).unwrap()),
        ("dialog", |s| {
            s.set_task_fields(TaskId(0), &[("name", "Renamed")])
                .unwrap();
        }),
        ("typed predecessors", |s| {
            s.edit_group(links(), Source::Typed).unwrap();
        }),
        ("undo links", |s| assert!(s.undo().unwrap())),
        ("redo links", |s| assert!(s.redo().unwrap())),
        ("paste", |s| {
            s.insert_many(&["Footing", "Slab"]).unwrap();
        }),
        ("delete first row", |s| {
            s.edit(Edit::Delete { task: TaskId(6) }).unwrap();
        }),
        ("delete second row", |s| {
            s.edit(Edit::Delete { task: TaskId(7) }).unwrap();
        }),
        ("undo second delete", |s| assert!(s.undo().unwrap())),
        ("set link", |s| {
            s.edit(Edit::SetLink {
                dep: DepId(0),
                kind: LinkType::SS,
                lag: 0,
            })
            .unwrap();
        }),
        ("data date", |s| {
            s.edit(Edit::SetDataDate { date: Some(1440) }).unwrap();
        }),
        ("undo data date", |s| assert!(s.undo().unwrap())),
        ("link release", |s| {
            let to = on(s, 3);
            link(s, 1, to);
        }),
        ("delete linked A", |s| {
            s.begin_whatif().unwrap();
            s.edit(Edit::Delete { task: TaskId(0) }).unwrap();
        }),
        ("delete linked B", |s| {
            s.edit(Edit::Delete { task: TaskId(1) }).unwrap();
        }),
        ("discard what-if", |s| s.discard_whatif().unwrap()),
        ("bar drag", |s| {
            let (x, y) = on(s, 2);
            s.pointer_down(x, y, false);
            s.pointer_move(x + 1500.0, y);
            s.pointer_up();
        }),
    ]
}

#[test]
fn every_handler_a_commit_calls_is_in_the_plans_needs() {
    let mut s = session();
    let seen = spy(&mut s, "");
    s.take_commits();
    for (label, action) in actions() {
        seen.borrow_mut().clear();
        action(&mut s);
        let wrote = commits(&mut s);
        assert!(!wrote.as_array().unwrap().is_empty(), "{label}");
        assert!(!seen.borrow().is_empty(), "{label}");
        for plan in seen.borrow().iter() {
            assert!(plan.needs.windows(2).all(|w| w[0] < w[1]), "{label}");
            for change in wrote
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|c| c["changes"].as_array().unwrap())
            {
                let (record, op) = (
                    change["record"].as_str().unwrap(),
                    change["op"].as_str().unwrap(),
                );
                let name = format!("{op}{}{}", record[..1].to_uppercase(), &record[1..]);
                assert!(plan.needs.contains(&name.as_str()), "{label}: {name}");
            }
        }
    }
}

#[test]
fn a_discard_needing_a_missing_link_handler_keeps_history_and_the_whatif() {
    let mut s = session();
    s.begin_whatif().unwrap();
    for task in [0, 1] {
        s.edit(Edit::Delete { task: TaskId(task) }).unwrap();
    }
    let before = (s.history_len(), s.whatif_summary());
    assert!(before.1.is_some());
    s.take_commits();
    spy(&mut s, "createLink");
    assert_eq!(
        s.discard_whatif(),
        Err(WhatIfError::Undo(EditError::Refused(REFUSED.into())))
    );
    assert_eq!((s.history_len(), s.whatif_summary()), before);
    assert_eq!(commits(&mut s), json!([]));
}

#[test]
fn a_discard_is_one_commit_and_its_put_back_takes_back_every_step() {
    let mut s = session();
    s.begin_whatif().unwrap();
    for task in [0, 1] {
        s.edit(Edit::Delete { task: TaskId(task) }).unwrap();
    }
    let deleted = snapshot(&s);
    s.take_commits();
    s.discard_whatif().unwrap();
    let sent = commits(&mut s);
    let sent = sent.as_array().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["source"], "undo");
    let creates = |op: &str| {
        let changes = sent[0]["changes"].as_array().unwrap().iter();
        changes
            .filter(|c| c["record"] == "task" && c["op"] == op)
            .count()
    };
    assert_eq!(creates("create"), 2);
    assert_eq!(s.history_len(), (0, 2));
    s.rollback_to(sent[0]["seq"].as_u64().unwrap()).unwrap();
    assert_eq!(snapshot(&s), deleted);
    assert_eq!(s.history_len(), (2, 0));
}

#[test]
fn refused_link_gestures_and_typed_predecessors_keep_redo() {
    let mut s = session();
    s.edit(longer(300)).unwrap();
    assert!(s.undo().unwrap());
    assert_eq!(s.history_len(), (0, 1));
    s.take_commits();
    spy(&mut s, "createLink");
    let to = on(&s, 2);
    link(&mut s, 0, to);
    assert_eq!(s.drag_note(), Some(REFUSED));
    assert_eq!(s.history_len(), (0, 1));
    assert_eq!(commits(&mut s), json!([]));
    s.set_before_edit(None);
    let (x, y) = on(&s, 0);
    s.pointer_down(x, y, true);
    let seen = spy(&mut s, "createLink");
    s.pointer_move(to.0, to.1);
    s.pointer_up();
    assert_eq!(seen.borrow()[0].kind, "linkRelease");
    assert_eq!(s.link_refusal(), Some(REFUSED));
    assert_eq!(s.history_len(), (0, 1));
    assert_eq!(commits(&mut s), json!([]));
    assert_eq!(
        s.edit_group(links(), Source::Typed),
        Err(EditError::Refused(REFUSED.into()))
    );
    assert_eq!(s.history_len(), (0, 1));
    assert_eq!(commits(&mut s), json!([]));
}

#[test]
fn a_data_date_undo_needs_the_schedule_handler() {
    let mut s = session();
    s.edit(Edit::SetDataDate { date: Some(1440) }).unwrap();
    let before = s.history_len();
    s.take_commits();
    spy(&mut s, "updateSchedule");
    assert!(matches!(s.undo(), Err(EditError::Refused(_))));
    assert_eq!(s.history_len(), before);
    assert_eq!(commits(&mut s), json!([]));
}
