//! A host that keeps each task's parent and `after` can rebuild the outline from an indent, an outdent or
//! a move's change set alone: every task whose parent or `after` the step changed is in it.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::reparent::{TREE, build};
use app::{Session, edit::EditError};
use engine::TaskId;
use serde_json::Value;
use std::collections::BTreeMap;

/// A host's rows: task slot to its parent and the sibling it follows.
type Host = BTreeMap<u32, (Option<u32>, Option<u32>)>;

/// The chart's outline as a host would hold it.
fn outline_of(session: &Session) -> Host {
    let outline = session.outline().unwrap();
    let mut host = Host::new();
    let lists = std::iter::once((None, outline.roots()))
        .chain((0..TREE.len() as u32).map(|t| (Some(t), outline.children(TaskId(t)))));
    for (parent, list) in lists {
        let mut after = None;
        for task in list {
            host.insert(task.0, (parent, after));
            after = Some(task.0);
        }
    }
    host
}

fn slot(reference: &Value) -> Option<u32> {
    reference["task"]
        .as_u64()
        .map(|t| u32::try_from(t).unwrap())
}

/// Select `task`, run `step`, and apply the commits it made to a host that held the outline before:
/// the host's rows, then the chart's.
fn host_after(task: u32, step: fn(&mut Session) -> Result<TaskId, EditError>) -> (Host, Host) {
    let mut session = build(&TREE, &[]);
    let mut host = outline_of(&session);
    session.select(task);
    step(&mut session).unwrap();
    let out: Value = serde_json::from_str(&session.take_commits()).unwrap();
    for commit in out["commits"].as_array().unwrap() {
        for change in commit["changes"].as_array().unwrap() {
            let row = host.get_mut(&u32::try_from(change["task"].as_u64().unwrap()).unwrap());
            match (change["field"].as_str(), row) {
                (Some("parent"), Some(row)) => row.0 = slot(&change["new"]),
                (Some("after"), Some(row)) => row.1 = slot(&change["new"]),
                _ => {}
            }
        }
    }
    (host, outline_of(&session))
}

#[test]
fn an_outdent_tells_the_host_which_sibling_now_follows_the_moved_task() {
    // b1 leaves B and follows it: C, which followed B, now follows b1.
    let (host, chart) = host_after(3, Session::outdent_selected);
    assert_eq!(host, chart);
}
