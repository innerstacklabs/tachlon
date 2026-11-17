//! The Near critical row filter (gs#703): the tasks with at most [`NEAR_DAYS`] working days of
//! total float, critical ones included.

use crate::edit::day_minutes;
use crate::{RowFilter, Session};
use engine::TaskId;

/// The float a task may have and still be near critical, in working days of its own calendar.
pub const NEAR_DAYS: i64 = 10;

impl Session {
    /// Whether the Near critical row filter is on.
    pub fn near_critical(&self) -> bool {
        self.row_filter == RowFilter::NearCritical
    }

    /// Turn the Near critical row filter on or off. Returns false when there is no outline or a
    /// bar press is active.
    pub fn set_near_critical(&mut self, on: bool) -> bool {
        let wanted = if on {
            RowFilter::NearCritical
        } else if self.row_filter == RowFilter::NearCritical {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// Whether `task`'s float is within [`NEAR_DAYS`] days of its calendar. Finished work is
    /// left out, as Critical leaves it (decision 0008): its float of 0 says nothing.
    pub(crate) fn near_critical_task(&self, task: TaskId) -> bool {
        let dates = &self.solution.dates[task.0 as usize];
        !dates.finished && dates.float <= NEAR_DAYS * day_minutes(self.schedule.calendar_of(task))
    }
}
