//! An insert, a paste, a delete and a move in the outline are commits that say which task appeared,
//! went or moved (decision 0019), read back from the outbox.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::{clock, collapse::standard_calendar};
use crate::link_gesture;
use app::{
    Place, Session, Siblings, edit::Edit, history::Source, meta::TaskMeta, outline::Outline,
};
use engine::{DepId, Limit, LinkType, Milestone, Pin, Schedule, Task, TaskId};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use timeline::Timeline;

const FIRST: TaskId = TaskId(0);

/// Unlinked one-day tasks in a flat outline, named A, B and so on, with the `keys` given; task 0
/// also has a color and a label.
fn outlined(keys: &[Option<&str>]) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let mut meta = Vec::new();
    for (i, key) in (0u8..).zip(keys) {
        let task = Task {
            duration: 480,
            calendar,
            snet: None,
            limit: Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).unwrap();
        let name = char::from(b'A' + i).to_string();
        let first = i == 0;
        meta.push(TaskMeta {
            name,
            outline_level: 1,
            key: key.map(Into::into),
            color: first.then_some([0x12, 0x34, 0x56, 255]),
            label: first.then(|| "Lbl".into()),
            ..TaskMeta::default()
        });
    }
    let solution = schedule.compute().unwrap();
    let ids: Vec<u32> = (0..u32::try_from(keys.len()).unwrap()).collect();
    let wbs: Vec<TaskId> = ids.iter().map(|&i| TaskId(i)).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).unwrap();
    let timeline = Timeline::new(ids, 1_000.0, 400.0, 1.0);
    let s = Session::new(schedule, solution, timeline, meta, clock);
    s.with_outline(outline).unwrap()
}

/// The commits read, as `(source, changes)`.
fn read(s: &mut Session) -> Vec<(String, Value)> {
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    let commit = |c: &Value| (c["source"].as_str().unwrap().into(), c["changes"].clone());
    out["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(commit)
        .collect()
}

fn commit(source: &str, changes: Value) -> (String, Value) {
    (source.into(), changes)
}

/// A task of one day with no limits or actuals, as a create writes it.
fn born(name: &str, after: Value, code: Value) -> Value {
    let mut fields = json!({"parent":null,"wbsCode":null,"calendar":{"calendar":0},"color":null,"label":null,"name":name,"snet":null,"duration":1.0,"actualStart":null,"actualFinish":null,"remaining":null,"snlt":null,"fnet":null,"fnlt":null,"mso":null,"mfo":null,"alap":false,"resume":null,"milestone":null});
    (fields["after"], fields["code"]) = (after, code);
    fields
}

fn create(key: Value, task: u32, fields: Value) -> Value {
    let mut change = json!({"record":"task","task":task,"op":"create"});
    (change["key"], change["fields"]) = (key, fields);
    change
}

fn delete(key: Value, task: u32) -> Value {
    let mut change = json!({"record":"task","task":task,"op":"delete"});
    change["key"] = key;
    change
}

/// Task A as a create writes it: `born`, with the color and label `outlined` gives it.
fn task_a() -> Value {
    let mut fields = born("A", Value::Null, Value::Null);
    fields["color"] = json!("#123456");
    fields["label"] = json!("Lbl");
    fields
}

#[test]
fn an_insert_below_a_keyed_task_is_a_create_and_its_undo_a_delete() {
    let mut s = outlined(&[Some("a1"), None, Some("c3")]);
    s.select_task(0);
    s.insert_below_selection().unwrap();
    assert!(s.undo().unwrap());
    let fields = born("New task", json!("a1"), json!("A1000"));
    let want = [
        commit("typed", json!([create(Value::Null, 3, fields)])),
        commit("undo", json!([delete(Value::Null, 3)])),
    ];
    assert_eq!(read(&mut s), want);
}

#[test]
fn a_paste_of_three_names_is_one_commit_of_three_creates_each_after_the_one_before() {
    let mut s = outlined(&[Some("a1"), None, Some("c3")]);
    s.select_task(0);
    s.insert_many(&["One", "Two", "Three"]).unwrap();
    let make = |task, name, after| {
        let code = json!(format!("A100{}", task - 3));
        create(Value::Null, task, born(name, after, code))
    };
    let want = json!([
        make(3, "One", json!("a1")),
        make(4, "Two", json!({"task":3})),
        make(5, "Three", json!({"task":4})),
    ]);
    assert_eq!(read(&mut s), [commit("paste", want)]);
}

#[test]
fn a_delete_of_a_keyed_task_its_undo_and_its_redo_carry_its_key() {
    let mut s = outlined(&[Some("a1"), None, Some("c3")]);
    s.select_task(0);
    s.delete_selected().unwrap();
    assert!(s.undo().unwrap() && s.redo().unwrap());
    let gone = json!([delete(json!("a1"), 0)]);
    let back = json!([create(json!("a1"), 0, task_a())]);
    let want = [
        commit("typed", gone.clone()),
        commit("undo", back),
        commit("redo", gone),
    ];
    assert_eq!(read(&mut s), want);
}

#[test]
fn a_created_task_writes_its_limits_actuals_color_and_label_in_the_host_names() {
    let mut s = outlined(&[Some("a1"), None]);
    let limit = Limit {
        snlt: Some(0),
        fnet: Some(480),
        fnlt: Some(2_000),
        pin: Some(Pin::Start(0)),
        alap: true,
        resume: Some(0),
        milestone: Some(Milestone::Finish),
    };
    s.edit(Edit::SetLimit { task: FIRST, limit }).unwrap();
    s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(480),
        actual_finish: None,
        remaining: Some(240),
    })
    .unwrap();
    s.select_task(0);
    s.delete_selected().unwrap();
    assert!(s.undo().unwrap());
    let mut want = task_a();
    for (field, value) in [
        ("snlt", json!("2026-01-05T00:00")),
        ("fnet", json!("2026-01-05T08:00")),
        ("fnlt", json!("2026-01-06T09:20")),
        ("mso", json!("2026-01-05T00:00")),
        ("alap", json!(true)),
        ("resume", json!("2026-01-05T00:00")),
        ("milestone", json!("finish")),
        ("actualStart", json!("2026-01-05T08:00")),
        ("remaining", json!(0.5)),
    ] {
        want[field] = value;
    }
    let made = read(&mut s).pop().unwrap();
    assert_eq!(made, commit("undo", json!([create(json!("a1"), 0, want)])));
}

/// Link A to B and B to C, run `delete`, then undo and redo it, each time checking that a copy of
/// the starting schedule kept from the records holds what the session does, each record once.
fn replayed_from_the_records(delete: impl Fn(&mut Session)) {
    let mut s = outlined(&[Some("a1"), None, Some("c3")]);
    let held = |s: &Session| -> BTreeSet<String> {
        let links = s.schedule().live_dependencies();
        let tasks = (0..3).filter(|&t| !s.schedule().is_deleted(TaskId(t)));
        let tasks = tasks.map(|t| format!("task{t}"));
        tasks
            .chain(links.map(|(dep, _)| format!("link{}", dep.0)))
            .collect()
    };
    let mut copy = held(&s);
    for (pred, succ) in [(0, 1), (1, 2)] {
        let (pred, succ, kind) = (TaskId(pred), TaskId(succ), LinkType::FS);
        s.edit(Edit::AddLink {
            pred,
            succ,
            kind,
            lag: 0,
        })
        .unwrap();
    }
    delete(&mut s);
    for step in ["delete", "undo", "redo"] {
        for change in read(&mut s).iter().flat_map(|(_, c)| c.as_array().unwrap()) {
            let record = change["record"].as_str().unwrap();
            let id = format!("{record}{}", change[record]);
            let deleting = change["op"] == "delete";
            let known = if deleting {
                copy.remove(&id)
            } else {
                !copy.insert(id.clone())
            };
            assert_eq!(known, deleting, "after the {step}: {record} {id}");
        }
        assert_eq!(copy, held(&s), "after the {step}");
        assert!(step == "redo" || if step == "undo" { s.redo() } else { s.undo() }.unwrap());
    }
}

#[test]
fn a_copy_made_from_the_records_matches_the_session_after_a_delete_its_undo_and_its_redo() {
    replayed_from_the_records(|s| {
        s.select_task(1);
        s.delete_selected().unwrap();
    });
}

#[test]
fn a_link_between_two_tasks_deleted_in_one_commit_is_deleted_and_created_once() {
    replayed_from_the_records(|s| {
        let both = vec![
            Edit::Delete { task: FIRST },
            Edit::Delete { task: TaskId(1) },
        ];
        s.edit_group(both, Source::Typed).unwrap();
    });
}

#[test]
fn a_copy_made_from_the_records_matches_the_session_after_a_commit_of_link_steps() {
    let add = Edit::AddLink {
        pred: FIRST,
        succ: TaskId(2),
        kind: LinkType::FS,
        lag: 0,
    };
    let (cut, back) = (
        |d| Edit::RemoveLink { dep: DepId(d) },
        |d| Edit::RestoreLink { dep: DepId(d) },
    );
    let end = Edit::Delete { task: TaskId(1) };
    let last = Edit::Delete { task: TaskId(2) };
    for steps in [
        vec![add.clone(), last.clone()],
        vec![cut(0), end.clone()],
        vec![add.clone(), cut(2), last],
        vec![add.clone(), cut(2), back(2)],
        vec![cut(0), back(0), cut(0)],
        vec![cut(0), back(0), end],
        vec![add, cut(2)],
        vec![cut(0), back(0)],
    ] {
        replayed_from_the_records(|s| {
            s.edit_group(steps.clone(), Source::Typed).unwrap();
        });
    }
}

fn up(task: u32, key: &Value, field: &str, old: &Value, new: &Value) -> Value {
    json!({"record":"task","op":"update","key":key,"task":task,"field":field,"old":old,"new":new})
}

fn reparent(s: &mut Session, task: u32, parent: Option<u32>, siblings: Siblings) {
    let place = Place {
        parent: parent.map(TaskId),
        after: None,
    };
    let task = TaskId(task);
    let edit = Edit::Reparent {
        task,
        place,
        siblings,
    };
    s.edit(edit).unwrap();
}

#[test]
fn an_indent_with_adopt_and_an_outdent_with_release_move_the_task_and_each_sibling() {
    let mut s = outlined(&[Some("a1"), None, Some("c3"), None]);
    reparent(&mut s, 1, Some(0), Siblings::Adopt(2));
    let place = Place {
        parent: None,
        after: Some(FIRST),
    };
    let (task, siblings) = (TaskId(1), Siblings::Release(2));
    let edit = Edit::Reparent {
        task,
        place,
        siblings,
    };
    s.edit(edit).unwrap();
    let (null, one) = (Value::Null, json!({"task":1}));
    let (a1, c3) = (json!("a1"), json!("c3"));
    let indent = json!([
        up(1, &null, "parent", &null, &a1),
        up(1, &null, "after", &a1, &null),
        up(2, &c3, "parent", &null, &one),
        up(2, &c3, "after", &one, &null),
        up(3, &null, "parent", &null, &one),
    ]);
    let outdent = json!([
        up(1, &null, "parent", &a1, &null),
        up(1, &null, "after", &null, &a1),
        up(2, &c3, "parent", &one, &null),
        up(2, &c3, "after", &null, &one),
        up(3, &null, "parent", &one, &null),
    ]);
    let want = [commit("typed", indent), commit("typed", outdent)];
    assert_eq!(read(&mut s), want);
}

#[test]
fn a_create_under_a_summary_names_its_parent_by_key_or_by_slot() {
    let mut s = outlined(&[Some("a1"), None, Some("c3")]);
    reparent(&mut s, 1, Some(0), Siblings::Keep);
    reparent(&mut s, 2, Some(1), Siblings::Keep);
    read(&mut s);
    s.select_task(1);
    s.insert_below_selection().unwrap();
    s.select_task(2);
    s.insert_below_selection().unwrap();
    let (mut under_key, mut under_slot) = (
        born("New task", json!({"task":1}), json!("A1000")),
        born("New task", json!("c3"), json!("A1001")),
    );
    under_key["parent"] = json!("a1");
    under_slot["parent"] = json!({"task":1});
    let want = [
        commit("typed", json!([create(Value::Null, 3, under_key)])),
        commit("typed", json!([create(Value::Null, 4, under_slot)])),
    ];
    assert_eq!(read(&mut s), want);
}

/// The one commit since the last read: its `source`, with at least one change.
fn one(s: &mut Session, source: &str) {
    let commits = read(s);
    let [(got, changes)] = &commits[..] else {
        panic!("{source}: {commits:?}")
    };
    assert!(
        got == source && !changes.as_array().unwrap().is_empty(),
        "{source}: {commits:?}"
    );
}

#[test]
fn every_edit_path_is_one_commit_with_its_source_and_a_change() {
    let mut s = outlined(&[Some("a1"), None, Some("c3"), None]);
    s.fit(0.0, 10_000.0);
    let task = FIRST;
    s.edit(Edit::Rename {
        task,
        name: "Pour".into(),
    })
    .unwrap();
    one(&mut s, "typed");
    assert!(s.undo().unwrap());
    one(&mut s, "undo");
    assert!(s.redo().unwrap());
    one(&mut s, "redo");
    let v = s.timeline().viewport.clone();
    let (x, y) = (v.x_of(1020.0) - 20.0, v.row_top(0) + v.row_height / 2.0);
    s.pointer_down(x, y, false);
    s.pointer_move(v.x_of(4_000.0), y);
    s.pointer_up();
    one(&mut s, "drag");
    s.insert_many(&["One", "Two"]).unwrap();
    one(&mut s, "paste");
    s.propose_duration(1, 3);
    s.accept_proposal();
    one(&mut s, "proposal");
    let to = link_gesture::on(&s, 2);
    link_gesture::link(&mut s, 0, to);
    one(&mut s, "drag");
    let limit = Limit {
        snlt: Some(0),
        ..Limit::default()
    };
    s.edit(Edit::SetLimit {
        task: TaskId(3),
        limit,
    })
    .unwrap();
    one(&mut s, "typed");
    s.select_task(3);
    s.insert_below_selection().unwrap();
    one(&mut s, "typed");
    s.delete_selected().unwrap();
    one(&mut s, "typed");
    reparent(&mut s, 3, Some(1), Siblings::Keep);
    one(&mut s, "typed");
}
