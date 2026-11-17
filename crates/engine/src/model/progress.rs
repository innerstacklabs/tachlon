//! Actual dates and remaining duration (gs#703, decision 0008): the setters, and the work the
//! passes schedule for a task.

use super::options::UnstartedRemaining;
use super::{Schedule, ScheduleError, Task, TaskId};
use crate::calendar::{Instant, Minutes};

impl Task {
    /// The working minutes the passes schedule: what remains once the task has started, or
    /// before it with `unstartedRemaining: on`, else the whole duration.
    #[inline]
    pub(crate) fn work(&self, unstarted: UnstartedRemaining) -> Minutes {
        match (self.actual_start, self.remaining, unstarted) {
            (Some(_), Some(r), _) | (None, Some(r), UnstartedRemaining::On) => r,
            _ => self.duration,
        }
    }
}

impl Schedule {
    /// Set or clear a task's actual start ([`Task::actual_start`]). Keeps the cached topological
    /// order.
    ///
    /// An actual changes the task's late dates as well as its early ones, which the cones
    /// ([`Schedule::recompute_cone`], [`Schedule::recompute_duration_cone_moved`]) do not
    /// re-derive for the changed task: call [`Schedule::compute`] after changing one.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task.
    pub fn set_actual_start(
        &mut self,
        task: TaskId,
        actual_start: Option<Instant>,
    ) -> Result<(), ScheduleError> {
        self.live_mut(task)?.actual_start = actual_start;
        Ok(())
    }

    /// Set or clear a task's actual finish ([`Task::actual_finish`]). Keeps the cached
    /// topological order. Call [`Schedule::compute`] after changing one, as for
    /// [`Schedule::set_actual_start`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task.
    pub fn set_actual_finish(
        &mut self,
        task: TaskId,
        actual_finish: Option<Instant>,
    ) -> Result<(), ScheduleError> {
        self.live_mut(task)?.actual_finish = actual_finish;
        Ok(())
    }

    /// Set or clear a task's remaining duration, in working minutes of its calendar
    /// ([`Task::remaining`]). Keeps the cached topological order. A negative remaining duration
    /// is refused with [`ScheduleError::NegativeDuration`] and nothing changes. Call
    /// [`Schedule::compute`] after changing one, as for [`Schedule::set_actual_start`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task and
    /// [`ScheduleError::NegativeDuration`] for a negative remaining duration.
    pub fn set_remaining(
        &mut self,
        task: TaskId,
        remaining: Option<Minutes>,
    ) -> Result<(), ScheduleError> {
        let t = self.live_mut(task)?;
        if remaining.is_some_and(|r| r < 0) {
            return Err(ScheduleError::NegativeDuration(task));
        }
        t.remaining = remaining;
        Ok(())
    }
}
