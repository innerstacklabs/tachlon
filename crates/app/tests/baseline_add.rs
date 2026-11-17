//! "Add new" to the baseline (gs#703, row 3 of the change-and-re-baseline plan): the tasks added
//! since the baseline get their current dates as baseline, and every other entry stays.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::Session;
use app::edit::{Edit, EditError};
use common::baseline::set;
use common::outline_session;
use engine::TaskId;
use std::sync::Arc;

const N: usize = 60;

/// A session with a baseline, one task inserted after it and a leaf lengthened so that something
/// slips. Returns the session and the inserted task.
fn with_added_and_slipped() -> (Session, TaskId) {
    let (mut session, _) = outline_session(N);
    set(&mut session);
    let place = session.place_below(None);
    let name = "Added after the baseline".to_string();
    session.edit(Edit::Insert { place, name }).expect("inserts");
    let added = TaskId(session.schedule().tasks().len() as u32 - 1);
    let outline = session.outline().unwrap();
    let leaf = *outline
        .preorder()
        .iter()
        .find(|&&t| !outline.is_summary(t) && outline.parent(t).is_some() && t != added)
        .expect("a nested leaf");
    let minutes = session.schedule().tasks()[leaf.0 as usize].duration + 480;
    session
        .edit(Edit::SetDuration {
            task: leaf,
            minutes,
        })
        .expect("a longer duration applies");
    (session, added)
}

#[test]
fn add_new_keeps_every_entry_and_dates_the_added_tasks() {
    let (mut session, added) = with_added_and_slipped();
    let before = Arc::clone(session.baseline().unwrap());
    let summary = session.baseline_diff().unwrap();
    assert_eq!(summary.added, 1, "one task is new");
    assert!(summary.slipped > 0, "something slipped");
    assert!(before.task(added).is_none());

    session.run_add_new_to_baseline().expect("adds");

    let after = session.baseline().unwrap();
    assert_eq!(after.len(), before.len() + 1);
    for i in 0..session.schedule().tasks().len() {
        let task = TaskId(i as u32);
        if task == added {
            continue;
        }
        assert_eq!(after.task(task), before.task(task), "task {i} is as it was");
    }
    let dates = &session.solution().dates[added.0 as usize];
    let entry = after.task(added).expect("the added task has an entry");
    assert_eq!(
        (entry.es, entry.ef, entry.float),
        (dates.es, dates.ef, Some(dates.float))
    );
    assert_eq!(after.project_finish(), before.project_finish());
    let now = session.baseline_diff().unwrap();
    assert_eq!(now.added, 0);
    assert_eq!(
        (
            now.slipped,
            now.finish_days,
            now.joined,
            now.left,
            now.removed
        ),
        (
            summary.slipped,
            summary.finish_days,
            summary.joined,
            summary.left,
            summary.removed
        ),
        "every recorded slip is kept"
    );
}

#[test]
fn undo_hands_back_the_same_snapshot() {
    let (mut session, _) = with_added_and_slipped();
    let before = Arc::clone(session.baseline().unwrap());
    session.run_add_new_to_baseline().expect("adds");
    assert!(!Arc::ptr_eq(session.baseline().unwrap(), &before));

    assert!(session.undo().unwrap());
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &before));
    assert_eq!(session.baseline_diff().unwrap().added, 1);

    assert!(session.redo().unwrap());
    assert_eq!(session.baseline_diff().unwrap().added, 0);
}

#[test]
fn add_new_with_no_baseline_is_refused() {
    let (mut session, _) = outline_session(N);
    assert!(matches!(
        session.run_add_new_to_baseline(),
        Err(EditError::NoBaseline)
    ));
    assert!(session.baseline().is_none());
}
