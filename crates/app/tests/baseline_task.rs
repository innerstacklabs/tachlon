//! Re-baseline one task (gs#703, engine step): its entry takes its current dates, the rest stay.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::Session;
use app::edit::Edit;
use common::{baseline::set, outline_session};
use engine::TaskId;
use std::sync::Arc;

/// A session with a data date and a baseline, and a nested leaf lengthened so that it slips.
fn slipped() -> (Session, TaskId) {
    let (mut session, _) = outline_session(60);
    session.set_data_date_text("2026-10-14").expect("sets");
    set(&mut session);
    let o = session.outline().unwrap();
    let leaf = o
        .preorder()
        .iter()
        .find(|&&t| !o.is_summary(t) && o.parent(t).is_some());
    let task = *leaf.expect("a nested leaf");
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 480;
    session
        .edit(Edit::SetDuration { task, minutes })
        .expect("applies");
    (session, task)
}

#[test]
fn reset_takes_the_current_dates_keeps_the_rest_and_undo_restores() {
    let (mut session, task) = slipped();
    let before = Arc::clone(session.baseline().unwrap());
    let now = session.solution().dates[task.0 as usize];
    assert_ne!(before.task(task).unwrap().ef, now.ef);

    session.run_rebaseline_task(task).expect("resets");

    let after = Arc::clone(session.baseline().unwrap());
    let entry = after.task(task).unwrap();
    assert_eq!(
        (entry.es, entry.ef, entry.float),
        (now.es, now.ef, Some(now.float))
    );
    for i in (0..session.schedule().tasks().len() as u32)
        .map(TaskId)
        .filter(|&t| t != task)
    {
        assert_eq!(after.task(i), before.task(i), "{i}");
    }
    assert_eq!(after.project_finish(), before.project_finish());
    assert_eq!(after.data_date(), before.data_date());
    assert!(session.undo().unwrap());
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &before));
}

#[test]
fn a_summary_a_deleted_task_and_an_entry_less_task_are_refused() {
    let (mut session, task) = slipped();
    let o = session.outline().unwrap();
    let summary = *o
        .preorder()
        .iter()
        .find(|&&t| o.is_summary(t))
        .expect("a summary");
    let place = session.place_below(None);
    let name = "Added after the baseline".to_string();
    session.edit(Edit::Insert { place, name }).expect("inserts");
    let added = TaskId(session.schedule().tasks().len() as u32 - 1);
    session.edit(Edit::Delete { task }).expect("deletes");
    let before = Arc::clone(session.baseline().unwrap());

    let kind = |s: &mut Session, t| s.run_rebaseline_task(t).unwrap_err().kind();
    assert_eq!(kind(&mut session, summary), "baselineOnSummary");
    assert_eq!(kind(&mut session, added), "notInBaseline");
    assert_eq!(kind(&mut session, task), "schedule");
    assert!(Arc::ptr_eq(session.baseline().unwrap(), &before));
}
