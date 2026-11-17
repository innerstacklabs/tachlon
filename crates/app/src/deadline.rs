//! The MS Project deadline of each task (gs#703), kept for display only. It is a side table beside
//! [`TaskMeta`](crate::meta::TaskMeta), indexed by task id: no date, float or link depends on it,
//! and no edit, undo or redo changes it. A task added after the open has none.

use crate::Session;
use engine::{Instant, TaskId};

impl Session {
    /// Keep the deadline of each task, as `Loaded::deadlines`: an instant from the schedule origin,
    /// `None` for a task whose file states none. Display only.
    pub fn with_deadlines(mut self, deadlines: Vec<Option<Instant>>) -> Session {
        self.deadlines = deadlines;
        self
    }

    /// The deadline the file gave `task`, or `None` when it gave none or the task is new.
    pub fn deadline(&self, task: TaskId) -> Option<Instant> {
        self.deadlines.get(task.0 as usize).copied().flatten()
    }
}
