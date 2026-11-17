//! The project must-finish-by date and the buffer it leaves (gantt#2074).
//!
//! [`Schedule::set_finish_by`] stores the date; the backward pass then runs from the earlier of it
//! and the computed finish. When the plan runs past the date, the tasks on
//! the late path read negative float; when it beats the date, the critical path stays at zero and
//! [`Schedule::buffer`] reports the time to spare.

use super::Schedule;
use crate::calendar::{DAY, Instant, Minutes};
use crate::cpm::Solution;

/// The time between the computed project finish and the must-finish-by date, positive when the
/// plan beats the date and negative when it runs past it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buffer {
    /// Working minutes of the calendar of the task that finishes last (the schedule's first
    /// calendar when several tasks finish last, or none does).
    pub minutes: Minutes,
    /// Whole calendar days, rounded toward zero.
    pub days: i64,
}

impl Schedule {
    /// Set or clear the must-finish-by date. Float is measured against the earlier of the date
    /// and the computed finish, so a date the plan cannot meet shows as negative float, never an
    /// engine error. Keeps the cached topological order. The date changes every late date, which
    /// the cones ([`Schedule::recompute_cone`], [`Schedule::recompute_duration_cone_moved`]) do
    /// not re-derive: call [`Schedule::compute`] after changing it.
    pub fn set_finish_by(&mut self, date: Option<Instant>) {
        self.finish_by = date;
        self.limited |= date.is_some();
    }

    /// The must-finish-by date, if one is set ([`Schedule::set_finish_by`]).
    #[must_use]
    pub fn finish_by(&self) -> Option<Instant> {
        self.finish_by
    }

    /// The instant the backward pass runs from: the earlier of the must-finish-by date and the
    /// computed finish `pf`, or `pf` when no date is set.
    pub(crate) fn anchor(&self, pf: Instant) -> Instant {
        self.finish_by.map_or(pf, |d| d.min(pf))
    }

    /// The buffer of `solution` against the must-finish-by date: `None` when no date is set or
    /// the schedule has no calendar. `solution` must be the result for this schedule.
    #[must_use]
    pub fn buffer(&self, solution: &Solution) -> Option<Buffer> {
        let date = self.finish_by?;
        let pf = solution.project_finish;
        let mut last = self
            .tasks
            .iter()
            .zip(&solution.dates)
            .filter(|(t, d)| !t.deleted && d.ef == pf)
            .map(|(t, _)| t.calendar);
        let cal = match (last.next(), last.next()) {
            (Some(c), None) => self.calendars.get(c.0 as usize),
            _ => self.calendars.first(),
        }?;
        Some(Buffer {
            minutes: cal.working_between(pf, date),
            days: (date - pf) / DAY,
        })
    }
}
