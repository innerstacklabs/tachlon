//! The Joined critical and Left critical row filters (gs#703): with a baseline set, the tasks that
//! are critical now and were not then, or the reverse. Their members are the ones
//! `Status.baseline` counts as `joined` and `left`, both read from [`engine::diff`].

use crate::baseline::Baseline;
use crate::{RowFilter, Session};
use engine::{SolutionDiff, TaskId};

impl Session {
    /// Whether the Joined critical row filter is on.
    pub fn joined_critical_only(&self) -> bool {
        self.row_filter == RowFilter::JoinedCritical
    }

    /// Turn the Joined critical row filter on or off. Returns false when there is no outline, no
    /// baseline (to turn it on), or a bar press is active.
    pub fn set_joined_critical_only(&mut self, on: bool) -> bool {
        self.set_path_filter(on, RowFilter::JoinedCritical)
    }

    /// Whether the Left critical row filter is on.
    pub fn left_critical_only(&self) -> bool {
        self.row_filter == RowFilter::LeftCritical
    }

    /// Turn the Left critical row filter on or off. Returns false when there is no outline, no
    /// baseline (to turn it on), or a bar press is active.
    pub fn set_left_critical_only(&mut self, on: bool) -> bool {
        self.set_path_filter(on, RowFilter::LeftCritical)
    }

    fn set_path_filter(&mut self, on: bool, filter: RowFilter) -> bool {
        let wanted = if on {
            if self.baseline.is_none() {
                return false;
            }
            filter
        } else if self.row_filter == filter {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// The tasks `diff` says joined and left the critical path, less the ones the summary does
    /// not count: a task that is deleted now or had no entry then, one the baseline has no float
    /// for (a file's baseline states dates only), and, for the tasks that left, one that is
    /// finished now (finished work is never critical, so it did not leave the path).
    fn path_changes(&self, baseline: &Baseline, diff: &SolutionDiff) -> (Vec<TaskId>, Vec<TaskId>) {
        let counted = |task: &&TaskId| {
            !self.schedule.is_deleted(**task)
                && baseline.task(**task).is_some_and(|t| t.float.is_some())
        };
        let joined = diff
            .joined_critical
            .iter()
            .filter(counted)
            .copied()
            .collect();
        let left = (diff.left_critical.iter().filter(counted))
            .filter(|t| !self.solution.get(**t).finished)
            .copied()
            .collect();
        (joined, left)
    }

    /// Which tasks are in the filter, by task id: the joined ones, or the left ones. Read when the
    /// rows are rebuilt with the filter on, as the Longest path members are.
    pub(crate) fn path_members(&self, joined: bool) -> Vec<bool> {
        let mut members = vec![false; self.schedule.tasks().len()];
        if let Some(baseline) = &self.baseline {
            let diff = engine::diff(baseline.as_solution(), &self.solution);
            let (now, then) = self.path_changes(baseline, &diff);
            for task in if joined { now } else { then } {
                members[task.0 as usize] = true;
            }
        }
        members
    }
}
