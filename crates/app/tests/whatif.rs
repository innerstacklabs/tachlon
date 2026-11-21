//! A what-if mark (gantt#703): begin, read, keep or discard.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Session, WhatIfError, edit::Edit, edit::EditError};
use engine::{DAY, TaskId};

/// Lengthen the task that finishes last by a working day.
fn lengthen(session: &mut Session) {
    let finish = session.solution().project_finish;
    let last = session.solution().dates.iter().position(|d| d.ef == finish);
    let task = TaskId(last.unwrap() as u32);
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 480;
    session.edit(Edit::SetDuration { task, minutes }).unwrap();
}

#[test]
fn discard_leaves_a_schedule_equal_to_a_full_compute_of_the_start() {
    let (mut session, _) = common::demo_session(60);
    let (start, stamp) = (session.solution().clone(), session.stamp());
    session.begin_whatif().unwrap();
    assert_ne!(session.stamp(), stamp, "begin moves the stamp");
    lengthen(&mut session);
    let summary = session.whatif_summary().unwrap();
    let days = |t: i64| t.div_euclid(DAY);
    let moved = days(session.solution().project_finish) - days(start.project_finish);
    assert_eq!((summary.edits, summary.finish_days), (1, moved));
    assert!(moved > 0 && summary.finish_now != summary.finish_was);
    let stamp = session.stamp();
    session.discard_whatif().unwrap();
    assert_ne!(session.stamp(), stamp, "discard moves the stamp");
    let full = session.schedule().clone().compute().unwrap();
    assert!(session.solution() == &full && session.solution() == &start);
    assert!(session.whatif_summary().is_none() && session.can_redo());
}

#[test]
fn an_undo_past_the_mark_ends_it() {
    let (mut session, _) = common::demo_session(60);
    lengthen(&mut session);
    session.begin_whatif().unwrap();
    session.undo().unwrap();
    assert!(session.whatif_summary().is_none());
}

#[test]
fn a_history_that_dropped_the_mark_cannot_be_discarded() {
    let mut session = common::demo_session(60).0.with_history_cap(2);
    session.begin_whatif().unwrap();
    assert_eq!(session.begin_whatif(), Err(WhatIfError::AlreadyOn));
    (0..3).for_each(|_| lengthen(&mut session));
    let solution = session.solution().clone();
    assert_eq!(session.discard_whatif(), Err(WhatIfError::Lost));
    assert!(session.solution() == &solution && session.history_len() == (2, 0));
    assert_eq!(session.keep_whatif(), Ok(()));
}

/// The commits in the outbox: `(seq, source, host token)`, oldest first.
fn outbox(session: &mut Session) -> Vec<(u64, String, Option<String>)> {
    let out: serde_json::Value = serde_json::from_str(&session.take_commits()).unwrap();
    let text = |c: &serde_json::Value, key: &str| c[key].as_str().map(str::to_owned);
    let commit = |c: &serde_json::Value| {
        (
            c["seq"].as_u64().unwrap(),
            text(c, "source").unwrap(),
            text(c, "host"),
        )
    };
    out["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(commit)
        .collect()
}

fn rename(session: &mut Session, task: u32) {
    let (task, name) = (TaskId(task), format!("n{task}"));
    session.edit(Edit::Rename { task, name }).unwrap();
}

#[test]
fn a_discard_is_one_commit_and_a_host_entry_in_it_keeps_its_own() {
    let mut session = common::demo_session(60).0;
    session.begin_whatif().unwrap();
    rename(&mut session, 1);
    rename(&mut session, 2);
    session.push_host_entry("lock-7").unwrap();
    rename(&mut session, 3);
    rename(&mut session, 4);
    outbox(&mut session);
    session.discard_whatif().unwrap();
    let sent: Vec<_> = outbox(&mut session)
        .into_iter()
        .map(|(_, s, h)| (s, h))
        .collect();
    let undo = ("undo".to_string(), None);
    assert_eq!(
        sent,
        [undo.clone(), ("undo".into(), Some("lock-7".into())), undo]
    );
    assert_eq!(session.history_len(), (0, 5));
}

/// The changes of the one commit a discard sends.
fn discard_changes(session: &mut Session) -> Vec<serde_json::Value> {
    outbox(session);
    session.discard_whatif().unwrap();
    let out: serde_json::Value = serde_json::from_str(&session.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 1);
    out["commits"][0]["changes"].as_array().unwrap().clone()
}

/// The first task with no task under it in the outline: a delete takes it alone.
fn leaf_of(meta: &[app::meta::TaskMeta]) -> TaskId {
    let at = (0..meta.len() - 1).find(|&i| meta[i + 1].outline_level <= meta[i].outline_level);
    TaskId(u32::try_from(at.unwrap()).unwrap())
}

#[test]
fn a_task_edited_then_deleted_comes_back_with_the_values_it_had_before_the_edit() {
    let (mut session, meta) = common::outline_session(30);
    let leaf = leaf_of(&meta);
    session.begin_whatif().unwrap();
    let name = "Renamed in the what-if".to_string();
    session.edit(Edit::Rename { task: leaf, name }).unwrap();
    session.edit(Edit::Delete { task: leaf }).unwrap();
    let changes = discard_changes(&mut session);
    let made = changes
        .iter()
        .find(|c| c["record"] == "task" && c["op"] == "create");
    assert_eq!(
        made.unwrap()["fields"]["name"],
        meta[leaf.0 as usize].name.as_str()
    );
}

#[test]
fn a_task_made_then_deleted_in_the_what_if_leaves_no_change() {
    let mut session = common::outline_session(30).0;
    session.begin_whatif().unwrap();
    let made = session.insert_many(&["Footing"]).unwrap()[0];
    session.edit(Edit::Delete { task: made }).unwrap();
    let changes = discard_changes(&mut session);
    let of_task = |c: &&serde_json::Value| c["record"] == "task" && c["task"] == made.0;
    assert!(changes.iter().find(of_task).is_none(), "{changes:#?}");
}

#[test]
fn a_field_changed_twice_in_the_what_if_is_one_change_from_its_value_before_the_discard() {
    let mut session = common::demo_session(60).0;
    let task = TaskId(2);
    session.begin_whatif().unwrap();
    let start = session.schedule().tasks()[2].duration;
    for minutes in [start + 480, start + 960] {
        session.edit(Edit::SetDuration { task, minutes }).unwrap();
    }
    let longest = session.schedule().tasks()[2].duration;
    let changes = discard_changes(&mut session);
    assert_eq!(session.schedule().tasks()[2].duration, start);
    let ours: Vec<_> = changes
        .iter()
        .filter(|c| c["task"] == 2 && c["field"] == "duration")
        .collect();
    assert_eq!(ours.len(), 1, "{changes:#?}");
    assert_ne!(longest, start);
    assert_ne!(ours[0]["old"], ours[0]["new"]);
}

#[test]
fn a_put_back_of_a_discard_the_cap_dropped_is_refused_whole() {
    let mut session = common::demo_session(200).0;
    session.begin_whatif().unwrap();
    (0..150).for_each(|task| rename(&mut session, task));
    let last = outbox(&mut session).last().unwrap().0;
    session.acknowledge(last);
    session.discard_whatif().unwrap();
    let discard = outbox(&mut session)[0].0;
    for _ in 0..51 {
        assert!(session.redo().unwrap());
    }
    assert_eq!(session.rollback_to(discard), Err(EditError::RollbackGone));
}

#[test]
fn begin_during_a_bar_press_is_refused() {
    let (mut session, _, _, point) = common::setup();
    session.pointer_down(point.0, point.1, false);
    assert_eq!(session.begin_whatif(), Err(WhatIfError::Busy));
}
