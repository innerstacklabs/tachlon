//! The host's before-edit function (gantt#2085, decision 0019, rules 5 and 6): the gate asks it once
//! per action, after read-only, and a refusal changes nothing; a plain edit, a group, a drag, the
//! dialog, a paste, undo, redo and a link drag carry the changes they would make.
#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::{link, on, session};
use crate::read_only::{longer, state, with_history};
use app::history::Source::{self, Proposal};
use app::{Session, edit::Edit, edit::EditError, host::Planned};
use engine::TaskId;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

type Asked = Rc<RefCell<Vec<Planned>>>;

/// Hand `s` a function that records what it is asked and refuses the kind `refused`.
fn spy(s: &mut Session, refused: &'static str) -> Asked {
    let asked = Asked::default();
    let seen = Rc::clone(&asked);
    s.set_before_edit(Some(Rc::new(move |plan: &Planned| {
        seen.borrow_mut().push(plan.clone());
        (plan.kind != refused)
            .then_some(())
            .ok_or(format!("no {refused}"))
    })));
    asked
}

fn rename(task: u32, name: &str) -> Edit {
    Edit::Rename {
        task: TaskId(task),
        name: name.into(),
    }
}

fn drag(s: &mut Session) {
    let (x, y) = on(s, 2);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 1500.0, y);
    s.pointer_up();
}

fn linked(s: &mut Session) {
    let to = on(s, 2);
    link(s, 0, to);
}

#[test]
fn a_refused_edit_changes_nothing_and_an_empty_reason_reads_as_the_default() {
    for (reason, shown) in [("locked", "locked"), ("", "This change is not allowed.")] {
        let mut s = session();
        let (before, _) = (state(&s), s.take_commits());
        let given = reason.to_string();
        s.set_before_edit(Some(Rc::new(move |_| Err(given.clone()))));
        let error = s.edit(longer(300)).unwrap_err();
        assert_eq!(error, EditError::Refused(reason.into()));
        assert_eq!((error.kind(), error.to_string()), ("refused", shown.into()));
        assert_eq!(state(&s), before);
        assert!(s.take_commits().starts_with(r#"{"commits":[],"#));
        s.set_read_only(true);
        assert_eq!(s.edit(longer(300)), Err(EditError::ReadOnly));
    }
}

#[test]
fn each_gated_action_asks_once_and_a_refusal_changes_nothing() {
    type Act = fn(&mut Session);
    let ten: [(&str, Act); 10] = [
        ("setDuration", |s| drop(s.edit(longer(300)))),
        ("group", |s| drop(s.edit_group(vec![longer(300)], Proposal))),
        ("setTaskFields", |s| {
            drop(s.set_task_fields(TaskId(0), &[("name", "X")]));
        }),
        ("insertMany", |s| drop(s.insert_many(&["Footing"]))),
        ("undo", |s| drop(s.undo())),
        ("redo", |s| drop(s.redo())),
        ("discardWhatIf", |s| drop(s.discard_whatif())),
        ("drag", drag),
        ("linkDrag", linked),
        ("linkRelease", linked),
    ];
    for (kind, act) in ten {
        let mut s = with_history();
        let asked = spy(&mut s, kind);
        let (before, _) = (state(&s), s.take_commits());
        act(&mut s);
        assert_eq!(state(&s), before, "{kind} changed the chart");
        assert!(s.take_commits().starts_with(r#"{"commits":[],"#), "{kind}");
        let count = asked.borrow().iter().filter(|p| p.kind == kind).count();
        assert_eq!(count, 1, "{kind} was asked {count} times");
        s.set_before_edit(None);
        act(&mut s);
        assert_ne!(state(&s), before, "{kind} with no function");
    }
}

#[test]
fn a_group_is_seen_in_order_with_the_first_old_value_and_the_last_new_one() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    let edits = vec![rename(0, "X"), longer(120), rename(0, "Y")];
    s.edit_group(edits, Proposal).unwrap();
    let plan = asked.borrow()[0].clone();
    assert_eq!((plan.kind, plan.tasks), ("group", vec![0]));
    assert_eq!(plan.source, Proposal);
    let name =
        r#"{"record":"task","op":"update","key":null,"task":0,"field":"name","old":"A","new":"Y"}"#;
    assert_eq!(plan.changes[0], name);
    assert!(plan.changes[1].contains(r#""field":"duration""#) && plan.changes.len() == 2);
}

#[test]
fn a_plan_names_the_changes_the_commit_then_writes() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    let start = Edit::Move {
        task: TaskId(2),
        snet: Some(600),
    };
    let edits = [longer(300), rename(1, "Renamed"), start];
    for (nth, edit) in edits.into_iter().enumerate() {
        s.take_commits();
        s.edit(edit).unwrap();
        let commits: Value = serde_json::from_str(&s.take_commits()).unwrap();
        let json = |change: &String| serde_json::from_str::<Value>(change).unwrap();
        let planned: Value = asked.borrow()[nth].changes.iter().map(json).collect();
        assert!(planned[0].is_object(), "edit {nth} carries a change");
        assert_eq!(planned, commits["commits"][0]["changes"]);
    }
}

/// The changes of the last plan the function was shown for `kind`.
fn shown(asked: &Asked, kind: &str) -> Vec<Value> {
    let asked = asked.borrow();
    let plan = asked.iter().rev().find(|p| p.kind == kind).unwrap();
    plan.changes.iter().map(|c| c.parse().unwrap()).collect()
}

/// The changes the last commit wrote.
fn written(s: &mut Session) -> Value {
    let commits: Value = s.take_commits().parse().unwrap();
    commits["commits"].as_array().unwrap().last().unwrap()["changes"].clone()
}

#[test]
fn a_drag_is_asked_once_with_the_start_it_would_set() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    let (x, y) = on(&s, 2);
    s.pointer_down(x, y, false);
    for frame in 1..=20 {
        s.pointer_move(x + 1500.0 * f64::from(frame), y);
    }
    s.pointer_up();
    let asks = asked.borrow().iter().filter(|p| p.kind == "drag").count();
    let first = shown(&asked, "drag");
    assert_eq!((asks, &first[0]["field"]), (1, &json!("snet")));
    assert!(first[0]["old"].is_null() && first[0]["new"].is_string());
    // A drag that ends on its first frame writes the commit the ask described.
    let mut one = session();
    one.pointer_down(x, y, false);
    one.pointer_move(x + 1500.0, y);
    one.pointer_up();
    assert_eq!(Value::from(first), written(&mut one));
}

#[test]
fn the_dialog_is_shown_each_field_it_sets_with_old_and_new() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    s.set_task_fields(TaskId(0), &[("name", " B "), ("duration", "5h")])
        .unwrap();
    let dialog = shown(&asked, "setTaskFields");
    assert_eq!(
        (&dialog[0]["old"], &dialog[0]["new"]),
        (&json!("A"), &json!("B"))
    );
    assert_eq!(
        (&dialog[1]["field"], dialog[1]["old"] != dialog[1]["new"]),
        (&json!("duration"), true)
    );
    assert_eq!(Value::from(dialog), written(&mut s));
}

#[test]
fn the_dialog_for_a_task_that_is_not_there_is_refused_with_a_function_set() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    let fields = [("duration", "5h"), ("snlt", "2026-01-05")];
    assert!(s.set_task_fields(TaskId(99), &fields).is_err());
    assert_eq!(shown(&asked, "setTaskFields"), Vec::<Value>::new());
}
#[test]
fn a_paste_is_shown_the_tasks_it_creates_with_their_names() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    let first = s.schedule().tasks().len();
    s.insert_many(&[" Footing ", "Slab"]).unwrap();
    let (paste, wrote) = (shown(&asked, "insertMany"), written(&mut s));
    for (n, name) in ["Footing", "Slab"].into_iter().enumerate() {
        let (made, wrote) = (&paste[n], &wrote[n]);
        assert_eq!(
            (&made["op"], &made["fields"]["name"]),
            (&json!("create"), &json!(name))
        );
        assert_eq!(
            (&made["task"], &made["fields"]["name"]),
            (&wrote["task"], &wrote["fields"]["name"])
        );
        assert_eq!(
            (&made["task"], made["fields"].get("old")),
            (&json!(first + n), None)
        );
    }
}

#[test]
fn undo_and_redo_are_shown_the_changes_the_step_replays() {
    let mut s = session();
    s.edit(longer(300)).unwrap();
    let asked = spy(&mut s, "never");
    for (kind, source) in [("undo", Source::Undo), ("redo", Source::Redo)] {
        assert!(if kind == "undo" { s.undo() } else { s.redo() }.unwrap());
        let plan = asked.borrow().last().unwrap().clone();
        assert_eq!(
            (plan.kind, plan.source, plan.tasks),
            (kind, source, vec![0])
        );
        let replay = shown(&asked, kind);
        assert_eq!(
            (&replay[0]["field"], replay[0]["old"] != replay[0]["new"]),
            (&json!("duration"), true)
        );
        assert_eq!(Value::from(replay), written(&mut s), "{kind}");
    }
}

#[test]
fn a_link_drag_is_shown_the_link_it_creates_with_from_and_to() {
    let mut s = session();
    let asked = spy(&mut s, "never");
    linked(&mut s);
    let made = shown(&asked, "linkRelease");
    let ends = (
        &made[0]["op"],
        &made[0]["fields"]["from"],
        &made[0]["fields"]["to"],
    );
    assert_eq!(
        ends,
        (&json!("create"), &json!({"task": 0}), &json!({"task": 2}))
    );
    assert_eq!(Value::from(made), written(&mut s));
}

#[test]
fn a_function_that_refuses_a_value_refuses_each_of_the_six_and_changes_nothing() {
    type Act = fn(&mut Session);
    let six: [(&str, Act); 6] = [
        ("drag", drag),
        ("setTaskFields", |s| {
            drop(s.set_task_fields(TaskId(0), &[("name", "X")]));
        }),
        ("insertMany", |s| drop(s.insert_many(&["Footing"]))),
        ("undo", |s| drop(s.undo())),
        ("redo", |s| drop(s.redo())),
        ("linkRelease", linked),
    ];
    for (kind, act) in six {
        let mut s = with_history();
        let asked = Asked::default();
        let log = Rc::clone(&asked);
        s.set_before_edit(Some(Rc::new(move |plan: &Planned| {
            log.borrow_mut().push(plan.clone());
            plan.changes
                .is_empty()
                .then_some(())
                .ok_or("a value".into())
        })));
        let (before, _) = (state(&s), s.take_commits());
        act(&mut s);
        assert!(!shown(&asked, kind).is_empty(), "{kind} carried no changes");
        assert_eq!(state(&s), before, "{kind} changed the chart");
        assert!(s.take_commits().starts_with(r#"{"commits":[],"#), "{kind}");
    }
}

/// The slots the function was last shown for `kind`.
fn slots(asked: &Asked, kind: &str) -> Vec<u32> {
    let asked = asked.borrow();
    asked
        .iter()
        .rev()
        .find(|p| p.kind == kind)
        .unwrap()
        .tasks
        .clone()
}

#[test]
fn undo_redo_and_discardwhatif_name_every_task_they_change() {
    let late = Edit::SetLimit {
        task: TaskId(3),
        limit: engine::Limit {
            alap: true,
            ..engine::Limit::default()
        },
    };
    let across = Edit::AddLink {
        pred: TaskId(1),
        succ: TaskId(2),
        kind: engine::LinkType::FS,
        lag: 0,
    };
    let steps = [
        ("a group", vec![rename(0, "X"), rename(1, "Y")], vec![0, 1]),
        ("a limit", vec![late.clone()], vec![3]),
        ("a link", vec![across], vec![1, 2]),
    ];
    for (name, mut edits, tasks) in steps {
        let mut s = session();
        if edits.len() == 1 {
            s.edit(edits.remove(0)).unwrap();
        } else {
            s.edit_group(edits, Proposal).unwrap();
        }
        let asked = spy(&mut s, "never");
        s.undo().unwrap();
        s.redo().unwrap();
        assert_eq!(slots(&asked, "undo"), tasks, "undo of {name}");
        assert_eq!(slots(&asked, "redo"), tasks, "redo of {name}");
    }
    let mut s = session();
    s.begin_whatif().unwrap();
    s.edit(rename(1, "Y")).unwrap();
    s.edit(late).unwrap();
    let asked = spy(&mut s, "never");
    s.discard_whatif().unwrap();
    assert_eq!(slots(&asked, "discardWhatIf"), vec![1, 3]);
}
