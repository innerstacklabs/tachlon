//! Why a schedule edit or computation failed.

use std::fmt;

use super::range::{RangeInput, RangeItem};
use super::{CalendarId, DepId, TaskId};
use crate::calendar::Instant;

/// Why a schedule edit or computation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    /// A calendar id with no calendar.
    UnknownCalendar(CalendarId),
    /// A task id with no task.
    UnknownTask(TaskId),
    /// A negative task duration.
    NegativeDuration(TaskId),
    /// A value outside the range its input allows (`MAX_DURATION`, `MAX_LAG`); nothing changed.
    OutOfRange {
        /// The input.
        input: RangeInput,
        /// The task or dependency.
        item: RangeItem,
        /// The refused value.
        value: i64,
    },
    /// A dependency of a task on itself.
    SelfDependency(TaskId),
    /// A dependency id with no dependency, or (for a remove) one already removed.
    UnknownDependency(DepId),
    /// A live dependency already links the same two tasks in the same
    /// direction; this is it.
    DuplicateDependency(DepId),
    /// The dependencies contain a cycle; this is one of them, in order
    /// (each task is a predecessor of the next, the last of the first).
    Cycle(Vec<TaskId>),
    /// No admissible start satisfies the task's constraints.
    Infeasible(TaskId),
    /// es > ls (or no late start at all): an engine bug, never an
    /// infeasibility, since the forward pass found es. `dep` is the successor dependency whose
    /// backward inequality fails at the early start (`None`: the project
    /// finish bound).
    InvariantViolated {
        /// The task.
        task: TaskId,
        /// The dependency, if one is to blame.
        dep: Option<DepId>,
        /// Early start.
        es: Instant,
        /// The latest start the backward constraints allow (`None`: no
        /// admissible start satisfies them at all).
        ls: Option<Instant>,
    },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleError::UnknownCalendar(c) => write!(f, "unknown calendar {}", c.0),
            ScheduleError::UnknownTask(t) => write!(f, "unknown {t}"),
            ScheduleError::NegativeDuration(t) => write!(f, "{t} has a negative duration"),
            ScheduleError::OutOfRange { input, item, value } => {
                write!(
                    f,
                    "{item} has a {input} of {value}, outside what the engine accepts"
                )
            }
            ScheduleError::SelfDependency(t) => write!(f, "{t} depends on itself"),
            ScheduleError::UnknownDependency(d) => write!(f, "unknown {d}"),
            ScheduleError::DuplicateDependency(d) => {
                write!(f, "{d} already links these tasks in this direction")
            }
            ScheduleError::Cycle(c) => {
                write!(f, "dependency cycle:")?;
                c.iter().try_for_each(|t| write!(f, " {}", t.0))
            }
            ScheduleError::Infeasible(t) => {
                write!(f, "{t}: no admissible start satisfies its constraints")
            }
            ScheduleError::InvariantViolated { task, dep, es, ls } => {
                match ls {
                    Some(ls) => write!(f, "engine bug: {task} has es {es} > ls {ls}")?,
                    None => write!(f, "engine bug: {task} has es {es} and no late start")?,
                }
                match dep {
                    Some(d) => write!(f, " ({d} fails at es)"),
                    None => write!(f, " (project finish bound fails at es)"),
                }
            }
        }
    }
}

impl std::error::Error for ScheduleError {}
