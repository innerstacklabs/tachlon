//! The row order of an opened P6 file (gs#865): the activities under one WBS node list by the
//! Start column the page shows, the engine's start, not by the start the file states.

use crate::meta::TaskMeta;
use engine::{TaskDates, TaskId};

/// `wbs` with each parent's P6 activities (a task with an activity ID) sorted by computed early
/// start. The sort is stable, so activities that start together keep the order they had (the
/// importer's activity-code order). Summaries and MS Project tasks keep their places: an MS
/// Project task has no activity ID, so a file of them comes back unchanged. Runs once per open.
pub(crate) fn by_computed_start(
    wbs: &[TaskId],
    meta: &[TaskMeta],
    dates: &[TaskDates],
) -> Vec<TaskId> {
    let mut slots = std::collections::HashMap::<Option<TaskId>, Vec<usize>>::default();
    for (slot, task) in wbs.iter().enumerate() {
        let meta = &meta[task.0 as usize];
        if meta.code.is_some() {
            slots.entry(meta.parent).or_default().push(slot);
        }
    }
    let mut ordered = wbs.to_vec();
    for positions in slots.values() {
        let mut tasks: Vec<TaskId> = positions.iter().map(|&slot| wbs[slot]).collect();
        tasks.sort_by_key(|task| dates[task.0 as usize].es);
        for (&slot, task) in positions.iter().zip(tasks) {
            ordered[slot] = task;
        }
    }
    ordered
}
