//! The range of every number the engine takes from outside (decision 0020, rule 1). No other
//! file states a bound for these inputs.

use std::fmt;

use super::{DepId, ScheduleError, TaskId};
use crate::calendar::{DAY, Instant, Minutes};

/// The latest date an instant input may hold, in minutes from the origin: 1,000 years of 366 days.
pub const MAX_DATE: Instant = 1000 * 366 * DAY;
/// The earliest date an instant input may hold: `-MAX_DATE`.
pub const MIN_DATE: Instant = -MAX_DATE;
/// The longest duration or remaining work: 100 years of 366 days, in working minutes.
pub const MAX_DURATION: Minutes = 100 * 366 * DAY;
/// The largest lag in either direction, in working minutes of the lag calendar.
pub const MAX_LAG: Minutes = MAX_DURATION;

/// The input a value was given for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeInput {
    /// A task's duration.
    Duration,
    /// A dependency's lag.
    Lag,
}

/// The task or dependency a refused value was given to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeItem {
    /// A task.
    Task(TaskId),
    /// A dependency; for a new one, the id it would have had.
    Dependency(DepId),
}

impl fmt::Display for RangeInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RangeInput::Duration => "duration",
            RangeInput::Lag => "lag",
        })
    }
}

impl fmt::Display for RangeItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeItem::Task(t) => t.fmt(f),
            RangeItem::Dependency(d) => d.fmt(f),
        }
    }
}

fn refuse(input: RangeInput, item: RangeItem, value: i64) -> ScheduleError {
    ScheduleError::OutOfRange { input, item, value }
}

/// A duration from 0 to [`MAX_DURATION`]: a negative one is [`ScheduleError::NegativeDuration`].
pub(crate) fn check_duration(task: TaskId, duration: Minutes) -> Result<(), ScheduleError> {
    if duration < 0 {
        Err(ScheduleError::NegativeDuration(task))
    } else if duration > MAX_DURATION {
        Err(refuse(
            RangeInput::Duration,
            RangeItem::Task(task),
            duration,
        ))
    } else {
        Ok(())
    }
}

/// A lag from `-MAX_LAG` to [`MAX_LAG`].
pub(crate) fn check_lag(dep: DepId, lag: Minutes) -> Result<(), ScheduleError> {
    if lag.unsigned_abs() > MAX_LAG.unsigned_abs() {
        Err(refuse(RangeInput::Lag, RangeItem::Dependency(dep), lag))
    } else {
        Ok(())
    }
}
