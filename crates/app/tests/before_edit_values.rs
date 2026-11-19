//! The before-edit function sees old and new values for the eleven edits that change no task field
//! (gantt#2085, decision 0019, rule 6), and a refusal changes nothing.
#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::link_gesture::session;
use crate::read_only::state;
use app::{Place, Session, Siblings, edit::Edit, edit::EditError, host::Planned};
use engine::{DepId, LinkType, TaskId};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// Hand `s` a function that records what it is asked and refuses when `refuse` is set.
fn watch(s: &mut Session, refuse: bool) -> Rc<RefCell<Vec<Planned>>> {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&asked);
    s.set_before_edit(Some(Rc::new(move |plan: &Planned| {
        seen.borrow_mut().push(plan.clone());
        if refuse {
            Err(EditError::Refused("no".into()))
        } else {
            Ok(())
        }
    })));
    asked
}

/// `edit`, on a chart `prepare` set up: the function is shown `needle`, and the changes the commit
/// then writes when `as_commit`; refused, the chart, the history and the commits are as they were.
fn sees(prepare: fn(&mut Session), edit: &Edit, needle: &str, as_commit: bool) {
    let mut s = session();
    prepare(&mut s);
    s.take_commits();
    let asked = watch(&mut s, false);
    s.edit(edit.clone()).unwrap();
    let planned = format!("[{}]", asked.borrow()[0].changes.join(","));
    assert!(planned.contains(needle), "{planned}");
    let wrote: Value = s.take_commits().parse().unwrap();
    if as_commit {
        assert_eq!(
            planned.parse::<Value>().unwrap(),
            wrote["commits"][0]["changes"]
        );
    }
    let mut s = session();
    prepare(&mut s);
    s.take_commits();
    let before = (state(&s), s.baseline().cloned());
    watch(&mut s, true);
    assert_eq!(s.edit(edit.clone()), Err(EditError::Refused("no".into())));
    assert_eq!((state(&s), s.baseline().cloned()), before);
    assert!(s.take_commits().starts_with(r#"{"commits":[],"#));
}

macro_rules! variants {
    ($($name:ident: $prepare:expr, $edit:expr, $needle:expr, $as_commit:expr;)*) => {
        $(#[test] fn $name() { sees($prepare, &$edit, $needle, $as_commit); })*
    };
}

fn none(_: &mut Session) {}
fn deleted(s: &mut Session) {
    s.edit(Edit::Delete { task: TaskId(2) }).unwrap();
}
fn unlinked(s: &mut Session) {
    s.edit(Edit::RemoveLink { dep: DepId(0) }).unwrap();
}
fn based(s: &mut Session) {
    s.edit(Edit::SetBaseline { baseline: None }).unwrap();
}
fn after(task: u32) -> Place {
    Place {
        parent: None,
        after: Some(TaskId(task)),
    }
}

variants! {
    an_insert_is_seen_as_a_create_with_its_place_and_name: none,
        Edit::Insert { place: after(3), name: "New".into() },
        r#""task":6,"op":"create","fields":{"parent":null,"after":{"task":3},"name":"New"}"#, false;
    a_delete_is_seen_as_the_task_and_its_links_going: none,
        Edit::Delete { task: TaskId(2) }, r#""link":1,"dayMinutes":1440,"op":"delete","fields":{"from":{"task":2},"to":{"task":3},"type":"FS","lag":0.0}},{"record":"task","key":null,"task":2,"dayMinutes":1440,"op":"delete","fields":{"parent":null,"after":{"task":1}"#, true;
    a_restore_is_seen_as_the_task_and_its_links_coming_back: deleted,
        Edit::Restore { task: TaskId(2), place: after(1) },
        r#""task":2,"dayMinutes":1440,"op":"create","fields":{"parent":null,"after":{"task":1}"#, true;
    a_reparent_is_seen_with_the_old_and_new_place_of_each_task_it_moves: none,
        Edit::Reparent { task: TaskId(4), place: after(0), siblings: Siblings::Release(1) },
        r#""task":4,"dayMinutes":1440,"field":"after","old":{"task":3},"new":{"task":0}"#, true;
    an_added_link_is_seen_as_a_create_with_its_type_and_lag: none,
        Edit::AddLink { pred: TaskId(1), succ: TaskId(2), kind: LinkType::SS, lag: 120 },
        r#""type":"SS","lag":0.08333333333333333"#, true;
    a_removed_link_is_seen_as_a_delete: none,
        Edit::RemoveLink { dep: DepId(0) }, r#""link":0,"dayMinutes":1440,"op":"delete""#, true;
    a_restored_link_is_seen_as_a_create: unlinked,
        Edit::RestoreLink { dep: DepId(0) }, r#""link":0,"dayMinutes":1440,"op":"create""#, true;
    a_set_link_is_seen_with_the_old_and_new_type_and_lag: none,
        Edit::SetLink { dep: DepId(0), kind: LinkType::SS, lag: 1440 },
        r#""field":"lag","old":0.0,"new":1.0"#, true;
    a_set_baseline_is_seen_as_none_then_the_baseline: none,
        Edit::SetBaseline { baseline: None },
        r#""field":"baseline","old":null,"new":{"projectFinish":""#, false;
    a_cleared_baseline_is_seen_as_the_baseline_then_none: based,
        Edit::ClearBaseline, r#""dataDate":null},"new":null}"#, false;
    a_data_date_is_seen_with_the_old_and_new_date: none,
        Edit::SetDataDate { date: Some(0) },
        r#""field":"dataDate","old":null,"new":"2026-01-05T00:00""#, true;
}
