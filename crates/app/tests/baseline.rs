//! The baseline (gs#703): set, clear, undo and redo, and what it keeps when the schedule changes.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::baseline::{Baseline, BaselineSummary, BaselineTask, SUMMARY_JSON_MAX};
use app::edit::{Edit, EditError};
use app::{RenderFacts, Session, load_bytes};
use common::baseline::{brute_force, live_set, set};
use common::outline_session;
use engine::TaskId;
use std::sync::Arc;
use xer::synth::{Spec, write};

const N: usize = 60;

fn leaf(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| !outline.is_summary(t) && outline.parent(t).is_some())
        .expect("a nested leaf")
}

fn lengthen(session: &mut Session, task: TaskId) {
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 480;
    session
        .edit(Edit::SetDuration { task, minutes })
        .expect("a longer duration applies");
}

#[test]
fn set_baseline_snapshots_every_live_task() {
    let (mut session, _) = outline_session(N);
    assert!(session.baseline().is_none(), "a new session has none");
    let gone = leaf(&session);
    session
        .edit(Edit::Delete { task: gone })
        .expect("a leaf deletes");

    let baseline = set(&mut session);

    let live = session.schedule().live_task_count();
    assert!(
        live < session.schedule().tasks().len(),
        "one task is deleted"
    );
    assert_eq!(baseline.len(), live);
    assert!(baseline.task(gone).is_none(), "a deleted task has no entry");
    for (i, dates) in session.solution().dates.iter().enumerate() {
        let task = TaskId(i as u32);
        if session.schedule().is_deleted(task) {
            continue;
        }
        let entry = baseline.task(task).expect("a live task has an entry");
        assert_eq!(
            (entry.es, entry.ef, entry.float),
            (dates.es, dates.ef, Some(dates.float))
        );
    }
    assert_eq!(baseline.project_finish(), session.solution().project_finish);
}

#[test]
fn an_edit_after_the_baseline_leaves_it_alone() {
    let (mut session, _) = outline_session(N);
    let baseline = set(&mut session);
    let before = (*baseline).clone();
    let task = leaf(&session);
    let start = session.solution().dates[task.0 as usize].ef;

    lengthen(&mut session, task);

    assert_ne!(
        session.solution().dates[task.0 as usize].ef,
        start,
        "the edit moved the task"
    );
    let now = session.baseline().expect("still set");
    assert!(Arc::ptr_eq(now, &baseline), "the same snapshot");
    assert_eq!(**now, before);
    assert_eq!(now.task(task).expect("an entry").ef, start);
}

#[test]
fn undo_of_set_restores_no_baseline() {
    let (mut session, _) = outline_session(N);
    let first = set(&mut session);
    assert_eq!(session.history_len(), (1, 0));

    assert_eq!(session.undo(), Ok(true));
    assert!(session.baseline().is_none());
    assert_eq!(session.redo(), Ok(true));
    assert!(
        Arc::ptr_eq(session.baseline().expect("redo sets it"), &first),
        "redo puts the same snapshot back"
    );

    // A second set replaces the first, and undo of it hands the first back, not a new snapshot.
    let task = leaf(&session);
    lengthen(&mut session, task);
    let second = set(&mut session);
    assert!(!Arc::ptr_eq(&first, &second) && *first != *second);
    assert_eq!(session.undo(), Ok(true));
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &first));

    // Clear and its undo.
    session
        .edit(Edit::ClearBaseline)
        .expect("a baseline clears");
    assert!(session.baseline().is_none());
    assert_eq!(session.undo(), Ok(true));
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &first));
    assert_eq!(session.redo(), Ok(true));
    assert!(session.baseline().is_none());
}

#[test]
fn clearing_with_no_baseline_is_refused_and_records_nothing() {
    let (mut session, _) = outline_session(N);
    assert_eq!(
        session.edit(Edit::ClearBaseline),
        Err(EditError::NoBaseline)
    );
    assert_eq!(session.history_len(), (0, 0));
}

#[test]
fn a_task_inserted_after_the_baseline_has_no_entry() {
    let (mut session, _) = outline_session(N);
    let baseline = set(&mut session);
    let count = baseline.len();

    let added = session.insert_below_selection().expect("a row inserts");

    assert!(session.schedule().tasks().len() > count);
    assert!(baseline.task(added).is_none());
    assert_eq!(session.baseline().expect("still set").len(), count);
}

#[test]
fn reopen_drops_the_baseline() {
    let (mut old, _) = outline_session(N);
    set(&mut old);
    let next = load_bytes(
        &write(&Spec::with_activities(2, 30)),
        &import::ProjectSelection::All,
        &|| 0.0,
    )
    .expect("a generated XER loads");

    let fresh = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            None,
            next.origin,
        )
        .expect("the outline matches its schedule");

    assert!(fresh.baseline().is_none());
    assert!(old.baseline().is_some(), "the old session keeps its own");
}

// The summary (gs#703, story 2b): `Session::baseline_diff` and `status.baseline`.

#[test]
fn the_summary_equals_the_diff_of_two_solutions() {
    let (mut session, _) = outline_session(N);
    assert_eq!(session.baseline_diff(), None, "no baseline, no summary");
    set(&mut session);
    let (then, live_then) = (session.solution().clone(), live_set(&session));
    let zero = brute_force(&session, &then, &live_then);
    assert_eq!(session.baseline_diff(), Some(zero), "right after the set");
    assert_eq!(zero, BaselineSummary::default());

    let leaves: Vec<TaskId> = {
        let outline = session.outline().unwrap();
        outline
            .preorder()
            .iter()
            .copied()
            .filter(|&t| !outline.is_summary(t) && outline.parent(t).is_some())
            .collect()
    };
    let mut seen = Vec::new();
    let mut check = |session: &Session, step: &str| {
        let got = session.baseline_diff().expect("a baseline is set");
        assert_eq!(got, brute_force(session, &then, &live_then), "{step}");
        seen.push(got);
    };

    for (i, &task) in leaves.iter().take(6).enumerate() {
        lengthen(&mut session, task);
        check(&session, &format!("lengthen #{i}"));
    }
    (0..20).for_each(|_| lengthen(&mut session, leaves[0]));
    session.insert_below_selection().expect("a row inserts");
    check(&session, "an insert");
    // A deleted slot reads as zero float in a solution: a task that had float then looks like one
    // that joined the critical path, and a critical one like one that left. Neither counts.
    let float_then = |t: &&TaskId| then.dates[t.0 as usize].float;
    let free = leaves.iter().rev().find(|t| float_then(t) > 0);
    let critical = leaves.iter().rev().find(|t| float_then(t) == 0);
    let (free, critical) = (
        free.expect("a leaf with float"),
        critical.expect("a critical leaf"),
    );
    for (task, step) in [
        (free, "delete a leaf with float"),
        (critical, "delete a critical leaf"),
    ] {
        session
            .edit(Edit::Delete { task: *task })
            .expect("a leaf deletes");
        check(&session, step);
    }
    for step in ["undo", "undo", "redo"] {
        let done = if step == "undo" {
            session.undo()
        } else {
            session.redo()
        };
        assert_eq!(done, Ok(true));
        check(&session, step);
    }

    // The chain has to have moved every count, or the equality above is a comparison of zeros.
    let most = |f: fn(&BaselineSummary) -> i64| seen.iter().map(f).max().unwrap();
    assert!(most(|s| i64::from(s.slipped)) > 0, "something slipped");
    assert!(most(|s| s.finish_days) > 0, "the finish moved");
    assert!(most(|s| i64::from(s.added)) > 0, "a row was added");
    assert!(most(|s| i64::from(s.removed)) > 0, "a row was removed");
    assert!(
        most(|s| i64::from(s.joined) + i64::from(s.left)) > 0,
        "the critical path changed"
    );
}

#[test]
fn a_drag_and_its_release_keep_the_summary_equal_to_the_diff() {
    let (mut session, task, _, point) = common::setup_with(true);
    set(&mut session);
    let (then, live_then) = (session.solution().clone(), live_set(&session));
    session.pointer_down(point.0, point.1, false);
    session.pointer_move(point.0 + common::TRAVEL_PX, point.1);
    session.apply_drag();
    let mid = session.baseline_diff().expect("set");
    assert_eq!(mid, brute_force(&session, &then, &live_then), "mid-drag");
    assert!(
        mid.slipped > 0,
        "the drag moved task {task} and its followers"
    );
    session.pointer_up();
    assert_eq!(
        session.baseline_diff(),
        Some(brute_force(&session, &then, &live_then)),
        "after the release"
    );
}

/// The text of the `baseline` key to the start of the next.
fn baseline_field(json: &str) -> &str {
    let at = json.find(r#","baseline":"#).expect("status has baseline");
    let end = json.find(r#","moved":"#).expect("status has moved");
    &json[at..end]
}

#[test]
fn status_baseline_is_null_or_the_summary_and_costs_bounded_bytes() {
    let facts = RenderFacts::default();
    let (mut session, _) = outline_session(N);
    let json = session.status_json(&facts);
    assert_eq!(baseline_field(&json), r#","baseline":null"#);
    let null_bytes = baseline_field(&json).len();

    set(&mut session);
    let json = session.status_json(&facts);
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    assert_eq!(
        value["baseline"],
        serde_json::json!({"slipped":0,"finish_days":0,"joined":0,"left":0,"added":0,"removed":0})
    );
    let set_bytes = baseline_field(&json).len();

    let task = leaf(&session);
    lengthen(&mut session, task);
    session.insert_below_selection().expect("a row inserts");
    let json = session.status_json(&facts);
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    let summary = session.baseline_diff().expect("set");
    assert!(summary.slipped > 0 && summary.added == 1);
    assert_eq!(value["baseline"]["slipped"], summary.slipped);
    assert_eq!(value["baseline"]["finish_days"], summary.finish_days);
    assert_eq!(value["baseline"]["added"], 1);
    assert_eq!(
        value["baseline"],
        serde_json::from_str::<serde_json::Value>(&summary.json()).unwrap()
    );
    let busy_bytes = baseline_field(&json).len();

    // Measured sizes, so a field that grows fails here; the bound holds for any six numbers.
    assert_eq!(null_bytes, 16);
    assert_eq!(set_bytes, 83);
    assert!(busy_bytes <= set_bytes + 6, "{busy_bytes} bytes");
    let widest = BaselineSummary {
        slipped: u32::MAX,
        finish_days: i64::MIN,
        joined: u32::MAX,
        left: u32::MAX,
        added: u32::MAX,
        removed: u32::MAX,
    };
    assert_eq!(widest.json().len(), SUMMARY_JSON_MAX);
}

#[test]
fn the_summary_is_computed_once_per_revision_and_never_without_a_baseline() {
    let facts = RenderFacts::default();
    let (mut session, _) = outline_session(N);
    for _ in 0..20 {
        assert_eq!(session.baseline_diff(), None);
        session.status_json(&facts);
    }
    let baseline = set(&mut session);
    assert_eq!(baseline.diffs_computed(), 0, "setting it computes nothing");
    for _ in 0..50 {
        session.status_json(&facts);
    }
    assert_eq!(baseline.diffs_computed(), 1, "50 frames, one computation");
    let revision = session.revision();

    let task = leaf(&session);
    lengthen(&mut session, task);
    assert_ne!(session.revision(), revision);
    for _ in 0..50 {
        session.status_json(&facts);
    }
    assert_eq!(baseline.diffs_computed(), 2, "an edit makes one more");

    // A scroll, a hover or a repeated read changes neither key.
    session.scroll_to_row(3);
    session.pointer_move(5.0, 5.0);
    session.status_json(&facts);
    assert_eq!(baseline.diffs_computed(), 2);

    assert_eq!(session.undo(), Ok(true));
    session.status_json(&facts);
    assert_eq!(baseline.diffs_computed(), 3, "an undo is a new revision");
}

#[test]
fn joined_and_left_skip_a_task_with_no_baseline_float() {
    let (mut session, _) = outline_session(N);
    let with_float = set(&mut session);
    // The same snapshot as a file would give it: dates, no float.
    let tasks = (0..session.schedule().tasks().len() as u32)
        .map(|i| {
            with_float
                .task(TaskId(i))
                .map(|t| BaselineTask { float: None, ..*t })
        })
        .collect();
    let without = Baseline::from_parts(tasks, with_float.project_finish());
    let task = leaf(&session);
    lengthen(&mut session, task);
    let known = session.baseline_diff().expect("set");
    assert!(
        known.joined + known.left > 0,
        "the edit changes the critical path, or this compares zeros: {known:?}"
    );

    session
        .edit(Edit::SetBaseline {
            baseline: Some(Arc::new(without)),
        })
        .expect("a baseline sets");
    let unknown = session.baseline_diff().expect("set");
    assert_eq!((unknown.joined, unknown.left), (0, 0));
    assert_eq!(
        BaselineSummary {
            joined: 0,
            left: 0,
            ..known
        },
        unknown,
        "everything else is read as before"
    );
}

#[test]
fn the_baseline_remembers_the_data_date_it_was_set_at() {
    let (mut session, _) = outline_session(N);
    let first = set(&mut session);
    assert_eq!(first.data_date(), None, "no data date, none remembered");
    assert_eq!(session.baseline_data_date_text(), None);

    session.set_data_date_text("2026-10-14").expect("sets");
    let at = session.data_date().expect("the data date is set");
    let second = set(&mut session);
    assert_eq!(second.data_date(), Some(at));
    assert_eq!(session.baseline_data_date_text().as_deref(), Some("14 Oct"));
    assert_ne!(
        first, second,
        "a re-set over the same rows is another snapshot"
    );

    // Moving the data date on does not move what the baseline says it was set at.
    session.set_data_date_text("2026-10-21").expect("moves");
    assert_eq!(session.baseline().unwrap().data_date(), Some(at));
    assert_eq!(session.baseline_data_date_text().as_deref(), Some("14 Oct"));

    // "Add new" keeps it.
    session.insert_below_selection().expect("inserts");
    session.run_add_new_to_baseline().expect("adds");
    assert_eq!(session.baseline().unwrap().data_date(), Some(at));

    // Undo puts back the snapshot with its own data date.
    for _ in 0..3 {
        assert!(session.undo().expect("undoes"));
    }
    assert_eq!(session.baseline().unwrap().data_date(), Some(at));
    assert!(session.undo().expect("undoes"), "the data date");
    assert!(session.undo().expect("undoes"), "the second set");
    assert_eq!(session.baseline().unwrap().data_date(), None);
}
