//! The Check row filter (gs#703): leaves counted by missing logic, open starts, open ends, high
//! float, high duration, invalid dates, missing resources, missed, hard constraints or negative
//! float, and their summaries.

use crate::{RowFilter, Session};
use engine::{Schedule, Task};

/// Which of the ten checks the filter keeps the leaves of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    /// Unfinished leaves with no predecessor or no successor.
    MissingLogic,
    /// Unfinished leaves with no predecessor.
    OpenStart,
    /// Unfinished leaves with no successor.
    OpenEnd,
    /// Tasks with more than 44 working days of float.
    HighFloat,
    /// Tasks longer than 44 working days.
    HighDuration,
    /// Tasks with an actual date after the data date.
    InvalidDates,
    /// Unfinished tasks with duration and no resource.
    MissingResources,
    /// Tasks that missed their baseline finish by the data date.
    Missed,
    /// Unfinished tasks a fixed date holds.
    HardConstraints,
    /// Unfinished tasks with total float below 0.
    NegativeFloat,
}

impl CheckKind {
    /// The name the page uses for this check.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::MissingLogic => "missing-logic",
            Self::OpenStart => "open-start",
            Self::OpenEnd => "open-end",
            Self::HighFloat => "high-float",
            Self::HighDuration => "high-duration",
            Self::InvalidDates => "invalid-dates",
            Self::MissingResources => "missing-resources",
            Self::Missed => "missed",
            Self::HardConstraints => "hard-constraints",
            Self::NegativeFloat => "negative-float",
        }
    }

    /// The check named `name`, or `None` for anything else.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::MissingLogic,
            Self::OpenStart,
            Self::OpenEnd,
            Self::HighFloat,
            Self::HighDuration,
            Self::InvalidDates,
            Self::MissingResources,
            Self::Missed,
            Self::HardConstraints,
            Self::NegativeFloat,
        ]
        .into_iter()
        .find(|kind| kind.name() == name)
    }

    /// Whether `task`, the slot `index`, is one this check counts. Whether it is a leaf is the
    /// caller's to say.
    pub(crate) fn holds(self, ends: &LinkEnds, index: usize, task: &Task) -> bool {
        let (start, end) = (!ends.has_pred[index], !ends.has_succ[index]);
        !task.deleted
            && task.actual_finish.is_none()
            && match self {
                Self::MissingLogic => start || end,
                Self::OpenStart => start,
                Self::OpenEnd => end,
                // The other seven are answered by `Session::health_members`.
                _ => false,
            }
    }
}

/// The key of `Session::health_members` that lists `kind`, for the seven it answers.
fn health_index(kind: CheckKind) -> Option<usize> {
    match kind {
        CheckKind::HighFloat => Some(3),
        CheckKind::HighDuration => Some(4),
        CheckKind::InvalidDates => Some(5),
        CheckKind::MissingResources => Some(6),
        CheckKind::Missed => Some(7),
        CheckKind::HardConstraints => Some(8),
        CheckKind::NegativeFloat => Some(9),
        _ => None,
    }
}

/// Which tasks, by slot, have a live link into them and a live link out of them.
pub(crate) struct LinkEnds {
    pub(crate) has_pred: Vec<bool>,
    pub(crate) has_succ: Vec<bool>,
}

impl LinkEnds {
    /// Reads the live links of `schedule`; a link with a deleted end is not one.
    pub(crate) fn of(schedule: &Schedule) -> LinkEnds {
        let mut ends = LinkEnds {
            has_pred: vec![false; schedule.tasks().len()],
            has_succ: vec![false; schedule.tasks().len()],
        };
        for (_, dep) in schedule.live_dependencies() {
            ends.has_succ[dep.pred.0 as usize] = true;
            ends.has_pred[dep.succ.0 as usize] = true;
        }
        ends
    }
}

impl Session {
    /// The check the Check row filter keeps, when that filter is on.
    pub fn check_filter(&self) -> Option<CheckKind> {
        match self.row_filter {
            RowFilter::Check(kind) => Some(kind),
            _ => None,
        }
    }

    /// Keep only the leaves `kind` counts, and the summaries above them; `None` turns the filter
    /// off and leaves a different filter on. It replaces any other row filter. Returns false when
    /// there is no outline or a bar press is active.
    pub fn set_check_filter(&mut self, kind: Option<CheckKind>) -> bool {
        let wanted = match (kind, &self.row_filter) {
            (Some(kind), _) => RowFilter::Check(kind),
            (None, RowFilter::Check(_)) => RowFilter::Off,
            (None, other) => other.clone(),
        };
        self.set_row_filter(wanted)
    }

    /// The tasks the Check filter keeps, by slot; `None` while it is off. Read when the rows are
    /// rebuilt with the filter on, as the Longest path members are.
    pub(crate) fn check_members(&self) -> Option<Vec<bool>> {
        let kind = self.check_filter()?;
        if let Some(key) = health_index(kind) {
            let mut members = vec![false; self.schedule.tasks().len()];
            for &slot in &self.health_members()[key] {
                members[slot as usize] = true;
            }
            return Some(members);
        }
        let ends = LinkEnds::of(&self.schedule);
        Some(
            (self.schedule.tasks().iter().enumerate())
                .map(|(index, task)| kind.holds(&ends, index, task))
                .collect(),
        )
    }
}
