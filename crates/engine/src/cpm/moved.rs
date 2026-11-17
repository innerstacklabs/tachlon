//! One entry of the moved list: a task whose early dates changed, and where they were.

use super::TaskDates;
use crate::calendar::Instant;
use crate::model::TaskId;

/// A task whose early start or early finish changed in one cone re-solve,
/// with the early dates it had before that edit (`was_es`, `was_ef`).
///
/// The new dates are in the [`Solution`](super::Solution) the call updated.
/// A caller that animates the change reads the old dates here, so it needs no
/// copy of the solution from before the edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    /// The task.
    pub task: TaskId,
    /// Its early start before the edit.
    pub was_es: Instant,
    /// Its early finish before the edit.
    pub was_ef: Instant,
}

impl Moved {
    /// `task`, which had `old` dates before the edit.
    pub(crate) fn was(task: u32, old: &TaskDates) -> Moved {
        Moved {
            task: TaskId(task),
            was_es: old.es,
            was_ef: old.ef,
        }
    }

    /// `task` with no old dates to report (the solution it replaced had the
    /// wrong length): `was_es` and `was_ef` equal its new dates, so nothing
    /// reads as having moved.
    pub(crate) fn unmoved(task: u32, new: &TaskDates) -> Moved {
        Moved::was(task, new)
    }
}
