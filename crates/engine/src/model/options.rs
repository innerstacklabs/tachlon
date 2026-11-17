//! Schedule options (decision 0018): settings that change what a schedule computes or reports.
//! Each is an enum with named values and one default constant; `name` and `from_name` are the
//! one table of their spellings, which the document and plain data use as they are.

use super::{Schedule, TaskId};
use crate::Instant;
use crate::cpm::TaskDates;

/// The late start reported for a started, unfinished task ([`Schedule::late_start_shown`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartedLateStart {
    /// The late start of the remaining work, which Primavera P6 shows.
    P6,
    /// The actual start, which Microsoft Project shows (as replicated by MPXJ).
    Project,
}

/// Whether an unstarted task's remaining duration sets its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnstartedRemaining {
    /// Remaining is ignored until the task starts.
    Off,
    /// An unstarted task with a remaining duration is scheduled with that length.
    On,
}

/// The default of `startedLateStart`.
pub const DEFAULT_STARTED_LATE_START: StartedLateStart = StartedLateStart::P6;
/// The default of `unstartedRemaining`.
pub const DEFAULT_UNSTARTED_REMAINING: UnstartedRemaining = UnstartedRemaining::Off;

/// The schedule options ([`Schedule::set_options`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleOptions {
    /// `startedLateStart`.
    pub started_late_start: StartedLateStart,
    /// `unstartedRemaining`.
    pub unstarted_remaining: UnstartedRemaining,
}

impl Default for ScheduleOptions {
    fn default() -> Self {
        Self {
            started_late_start: DEFAULT_STARTED_LATE_START,
            unstarted_remaining: DEFAULT_UNSTARTED_REMAINING,
        }
    }
}

impl StartedLateStart {
    /// The value as the document and plain data spell it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::P6 => "p6",
            Self::Project => "project",
        }
    }

    /// The value `name` spells, if it is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::P6, Self::Project]
            .into_iter()
            .find(|v| v.name() == name)
    }
}

impl UnstartedRemaining {
    /// The value as the document and plain data spell it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }

    /// The value `name` spells, if it is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Off, Self::On].into_iter().find(|v| v.name() == name)
    }
}

impl Schedule {
    /// Set the schedule options. Call [`Schedule::compute`] after changing one:
    /// `unstartedRemaining` changes the solved dates, which the cones do not re-derive.
    pub fn set_options(&mut self, options: ScheduleOptions) {
        self.options = options;
    }

    /// The schedule options.
    #[must_use]
    pub fn options(&self) -> ScheduleOptions {
        self.options
    }

    /// The late start to report for a task with solved `dates`. Under `startedLateStart:
    /// project` a started, unfinished task reports its actual start; every other task reports
    /// what the solution holds. Float and the critical path read the solution, not this.
    #[must_use]
    pub fn late_start_shown(&self, task: TaskId, dates: &TaskDates) -> Instant {
        let started = self
            .tasks
            .get(task.0 as usize)
            .is_some_and(|t| t.actual_start.is_some());
        let started = started && !dates.finished;
        match self.options.started_late_start {
            StartedLateStart::Project if started => dates.es,
            _ => dates.ls,
        }
    }
}
