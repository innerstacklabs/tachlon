//! What differs between two solutions (gs#703): the change in each task's
//! dates and float, who joined or left the critical path, the change in the
//! project finish, and how many tasks were added or removed.
//!
//! A pure function of two [`Solution`]s that an app calls once per revision
//! (against a baseline, say). It reads nothing but the two solutions, so
//! the result is exactly what a per-task subtraction of two full
//! [`compute`](crate::Schedule::compute) results gives.

use crate::calendar::Minutes;
use crate::cpm::Solution;
use crate::model::TaskId;

/// The change in one task that is in both solutions: new minus old.
///
/// Unit: the engine's time unit, minutes. `start` and `finish` are wall-clock
/// minutes ([`Instant`](crate::Instant) differences). `float` is working
/// minutes of the task's own calendar, as [`TaskDates::float`](crate::TaskDates::float)
/// is. A solution does not carry the calendar, so a change cannot be
/// converted to working days here; a caller that wants days divides `float`
/// by the working minutes in a day of that task's calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskChange {
    /// The task.
    pub task: TaskId,
    /// Change in early start, in minutes. Positive means later.
    pub start: Minutes,
    /// Change in early finish, in minutes. Positive means later.
    pub finish: Minutes,
    /// Change in total float, in working minutes of the task's calendar.
    /// Positive means more float.
    pub float: Minutes,
}

/// What differs between an old and a new [`Solution`], from [`diff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolutionDiff {
    /// One entry per task id present in both solutions, in id order, whether
    /// or not it changed.
    pub changes: Vec<TaskChange>,
    /// Tasks present in both that are critical now ([`TaskDates::is_critical`](crate::TaskDates::is_critical)) and were not before.
    pub joined_critical: Vec<TaskId>,
    /// Tasks present in both that were critical before ([`TaskDates::is_critical`](crate::TaskDates::is_critical)) and are not now.
    pub left_critical: Vec<TaskId>,
    /// Change in the project finish, in minutes. Positive means later.
    pub project_finish: Minutes,
    /// Ids present in the new solution only (it has more slots).
    pub added: usize,
    /// Ids present in the old solution only (it has fewer slots).
    pub removed: usize,
}

/// Compare `old` with `new`, matching tasks by id.
///
/// An id in both solutions gets a [`TaskChange`] and a critical-path check;
/// ids past the end of the shorter solution are only counted
/// ([`SolutionDiff::added`], [`SolutionDiff::removed`]).
///
/// A [`Solution`] has a slot for every task id ever made. A deleted task
/// keeps its slot, all zero, which reads as a task with no float; this
/// function cannot tell it from a live one. A caller that deletes tasks
/// between the two solutions reads the deleted ids from the schedule.
#[must_use]
pub fn diff(old: &Solution, new: &Solution) -> SolutionDiff {
    let both = old.dates.len().min(new.dates.len());
    let mut changes = Vec::with_capacity(both);
    let mut joined_critical = Vec::new();
    let mut left_critical = Vec::new();
    for (i, (o, n)) in old.dates.iter().zip(&new.dates).enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "i indexes the shorter solution, one entry per task: a slot below the task \
                      count, which TaskId holds in u32 (unchecked at Schedule::add_task)"
        )]
        let task = TaskId(i as u32);
        changes.push(TaskChange {
            task,
            start: n.es - o.es,
            finish: n.ef - o.ef,
            float: n.float - o.float,
        });
        match (o.is_critical(), n.is_critical()) {
            (false, true) => joined_critical.push(task),
            (true, false) => left_critical.push(task),
            _ => {}
        }
    }
    SolutionDiff {
        changes,
        joined_critical,
        left_critical,
        project_finish: new.project_finish - old.project_finish,
        added: new.dates.len() - both,
        removed: old.dates.len() - both,
    }
}
