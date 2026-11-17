//! The Status row filter (gs#703): the leaves that are In progress, Not started or Complete, by
//! the card's rule.

use crate::{RowFilter, Session};
use engine::Task;

/// A task's state as the card reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    InProgress,
    NotStarted,
    Complete,
}

impl StatusKind {
    /// The name the page uses for this state.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::InProgress => "in-progress",
            Self::NotStarted => "not-started",
            Self::Complete => "complete",
        }
    }

    /// The state named `name`, or `None` for anything else.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::InProgress, Self::NotStarted, Self::Complete]
            .into_iter()
            .find(|kind| kind.name() == name)
    }

    /// Whether `task` is in this state: Complete has an actual finish, In progress an actual
    /// start and no actual finish, Not started neither.
    pub(crate) fn holds(self, task: &Task) -> bool {
        match self {
            Self::Complete => task.actual_finish.is_some(),
            Self::InProgress => task.actual_start.is_some() && task.actual_finish.is_none(),
            Self::NotStarted => task.actual_start.is_none() && task.actual_finish.is_none(),
        }
    }
}

impl Session {
    /// The state the Status row filter keeps, when that filter is on.
    pub fn status_filter(&self) -> Option<StatusKind> {
        match self.row_filter {
            RowFilter::Status(kind) => Some(kind),
            _ => None,
        }
    }

    /// Keep only the leaves in state `kind`, and the summaries above them; `None` turns the
    /// filter off and leaves a different filter on. It replaces any other row filter. Returns
    /// false when there is no outline or a bar press is active.
    pub fn set_status_filter(&mut self, kind: Option<StatusKind>) -> bool {
        let wanted = match (kind, &self.row_filter) {
            (Some(kind), _) => RowFilter::Status(kind),
            (None, RowFilter::Status(_)) => RowFilter::Off,
            (None, other) => other.clone(),
        };
        self.set_row_filter(wanted)
    }
}
