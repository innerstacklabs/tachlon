//! The date limits a task can carry beyond start-no-earlier-than (gantt#1875).

use super::range::check_limit;
use super::{Schedule, ScheduleError, TaskId};
use crate::calendar::Instant;

/// Start-no-later-than, finish-no-earlier-than, finish-no-later-than and a [`Pin`], each optional.
/// `fnet` raises the finish floor, as a finish-to-finish bound does. `snlt` and `fnlt` cap the
/// late start and late finish; one no date can meet gives a late start before the early start,
/// so a negative float; the late start can then fall before the origin, where the weekly pattern
/// runs on.
/// A started task ignores `snlt` and a start pin, as it ignores start-no-earlier-than, and a
/// finished task ignores all of them (decision 0008). A pin and a limit on one side both bind:
/// the stricter wins. `alap` places an unstarted task as late as possible without delaying
/// anything after it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limit {
    /// Start-no-later-than: the late start is no later.
    pub snlt: Option<Instant>,
    /// Finish-no-earlier-than: the early finish is no earlier.
    pub fnet: Option<Instant>,
    /// Finish-no-later-than: the late finish is no later.
    pub fnlt: Option<Instant>,
    /// Must start on, or must finish on, a date.
    pub pin: Option<Pin>,
    /// As late as possible: the early start moves to the latest start that delays no successor
    /// (its free float is used, not its total), as P6 and MS Project place it. A task with no
    /// successor starts as late as the project finish allows. A started task ignores it.
    pub alap: bool,
    /// The date a started task's remaining work picks up again: its early start is no earlier.
    /// Ignored until the task has an actual start, and for a finished task. A resume before the
    /// data date changes nothing.
    pub resume: Option<Instant>,
}

/// A date a task must start on or finish on, as P6 and MS Project files carry one. The date
/// holds whatever the tasks before it do: no dependency binds the task, and a predecessor that
/// cannot finish in time gets a negative float instead. The data date and start-no-earlier-than
/// still bound it from below; past its date, the pinned task shows the negative float.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    /// Must start on this instant (the next working one when it is not).
    Start(Instant),
    /// Must finish on this instant (the end of the last working minute at or before it when
    /// none ends there).
    Finish(Instant),
}

impl Limit {
    /// The start pin, which a started task ignores.
    pub(crate) fn start_pin(&self, started: bool) -> Option<Instant> {
        match self.pin {
            Some(Pin::Start(c)) if !started => Some(c),
            _ => None,
        }
    }

    /// The finish pin.
    pub(crate) fn finish_pin(&self) -> Option<Instant> {
        match self.pin {
            Some(Pin::Finish(c)) => Some(c),
            _ => None,
        }
    }
}

impl Schedule {
    /// Set a task's date limits ([`Task::limit`](super::Task::limit)). Keeps the cached
    /// topological order. A limit changes late dates the cones ([`Schedule::recompute_cone`],
    /// [`Schedule::recompute_duration_cone_moved`]) do not re-derive for the changed task: call
    /// [`Schedule::compute`] after changing one. A schedule with an `alap` task takes a full
    /// compute from every cone too.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task and
    /// [`ScheduleError::OutOfRange`] for a date, a pin included, out of range.
    pub fn set_limit(&mut self, task: TaskId, limit: Limit) -> Result<(), ScheduleError> {
        let t = self.live_mut(task)?;
        check_limit(task, &limit)?;
        t.limit = limit;
        self.limited |= limit != Limit::default();
        Ok(())
    }
}
