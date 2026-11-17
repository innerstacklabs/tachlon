//! The Float lost row filter (gs#703): the tasks whose total float is at least [`LOST_DAYS`]
//! working days of their own calendar below what the baseline held, and the summaries above them.

use crate::edit::day_minutes;
use crate::{RowFilter, Session};
use engine::TaskId;

/// The float a task must have lost since the baseline, in working days of its own calendar.
pub const LOST_DAYS: i64 = 5;

impl Session {
    /// Whether the Float lost row filter is on.
    pub fn float_lost(&self) -> bool {
        self.row_filter == RowFilter::FloatLost
    }

    /// Turn the Float lost row filter on or off. Returns false when turning it on with no baseline
    /// or one that holds no float (a baseline read from a file), when there is no outline, or when
    /// a bar press is active.
    pub fn set_float_lost(&mut self, on: bool) -> bool {
        let wanted = if on {
            if !self.baseline_has_float() {
                return false;
            }
            RowFilter::FloatLost
        } else if self.row_filter == RowFilter::FloatLost {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// Whether the baseline in force holds float for any task.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    pub fn baseline_has_float(&self) -> bool {
        self.baseline.as_ref().is_some_and(|baseline| {
            (0..self.schedule.tasks().len()).any(|i| {
                baseline
                    .task(TaskId(i as u32))
                    .is_some_and(|t| t.float.is_some())
            })
        })
    }

    /// Whether `task` has [`LOST_DAYS`] days of its calendar less float than the baseline held.
    /// A task the baseline has no entry for, or no float for, is not kept; finished work is left
    /// out, as Critical and Near critical leave it (decision 0008).
    pub(crate) fn float_lost_task(&self, task: TaskId) -> bool {
        let dates = &self.solution.dates[task.0 as usize];
        let then = self
            .baseline
            .as_ref()
            .and_then(|baseline| baseline.task(task))
            .and_then(|then| then.float);
        !dates.finished
            && then.is_some_and(|then| {
                then - dates.float >= LOST_DAYS * day_minutes(self.schedule.calendar_of(task))
            })
    }

    /// The filter reads the baseline's float, so it goes off when there is none. Called when the
    /// rows are rebuilt, which follows every change of baseline.
    pub(crate) fn end_float_lost_without_float(&mut self) {
        if self.row_filter == RowFilter::FloatLost && !self.baseline_has_float() {
            self.row_filter = RowFilter::Off;
        }
    }
}
