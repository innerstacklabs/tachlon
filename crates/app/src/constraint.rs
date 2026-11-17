//! Which tasks carry a real start constraint (gs#703 K2).
//!
//! An import floors every task's start-no-earlier-than at its project's start, so almost every
//! imported task has a `snet`. Only one strictly above the task's floor is a constraint.

use crate::Session;
use engine::{Instant, Limit, Pin, Task, TaskId};

/// The constraint a task with start-no-earlier-than `snet` and floor `floor` carries: `snet` when
/// it is strictly above the floor, or when the task has no floor, else `None`.
pub(crate) fn constraint_of(snet: Option<Instant>, floor: Option<Instant>) -> Option<Instant> {
    let at = snet?;
    match floor {
        Some(floor) if at <= floor => None,
        _ => Some(at),
    }
}

/// The floor a task saved without one gets back, and so the floor a document leaves out: the
/// earliest start-no-earlier-than among `tasks`. It is what an import floors every task at when one
/// project holds them all (an MS Project file, a one-project XER). `None` when no task has one.
pub(crate) fn fallback_floor<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Option<Instant> {
    tasks.into_iter().filter_map(|task| task.snet).min()
}

impl Session {
    /// The start-no-earlier-than date `task` carries as a real constraint, or `None`: it has none,
    /// holds only its import floor, has an actual start or finish (its own start-no-earlier-than no
    /// longer binds, decision 0008), or is not a task. A task with no floor (the demo, a task
    /// inserted here, an XER summary) carries every `snet` it has.
    #[must_use]
    pub fn start_constraint(&self, task: TaskId) -> Option<Instant> {
        let index = task.0 as usize;
        let task = self.schedule.tasks().get(index)?;
        if task.actual_start.is_some() || task.actual_finish.is_some() {
            return None;
        }
        constraint_of(task.snet, self.meta.get(index)?.floor)
    }

    /// Whether `task` is unfinished and, when `unstarted` is asked for, has no actual start, and
    /// `holds` its date limits.
    fn open_limit(&self, task: u32, unstarted: bool, holds: impl FnOnce(&Limit) -> bool) -> bool {
        self.schedule.tasks().get(task as usize).is_some_and(|t| {
            t.actual_finish.is_none() && !(unstarted && t.actual_start.is_some()) && holds(&t.limit)
        })
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl timeline::scene::StartConstraints for Session {
    /// Whether the scene marks the left edge of `task`: it carries a real start-no-earlier-than
    /// ([`Session::start_constraint`]), or a start-no-later-than or a start pin while it has no
    /// actual start (a started task ignores both, a finished one every limit).
    fn holds_start(&self, task: u32) -> bool {
        self.start_constraint(TaskId(task)).is_some()
            || self.open_limit(task, true, |limit| {
                limit.snlt.is_some() || matches!(limit.pin, Some(Pin::Start(_)))
            })
    }

    /// Whether the scene marks the right edge of `task`: a finish limit (no earlier or no later
    /// than) or a finish pin, while it has no actual finish.
    fn holds_finish(&self, task: u32) -> bool {
        self.open_limit(task, false, |limit| {
            limit.fnet.is_some()
                || limit.fnlt.is_some()
                || matches!(limit.pin, Some(Pin::Finish(_)))
        })
    }
}
