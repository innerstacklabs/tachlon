//! The range of every number the engine takes from outside (decision 0020, rule 1). No other
//! file states a bound for these inputs.

use std::fmt;

use super::{DepId, Limit, ScheduleError, Task, TaskId};
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
    /// A task's remaining duration.
    Remaining,
    /// A task's start-no-earlier-than date.
    Snet,
    /// A task's start-no-later-than date.
    Snlt,
    /// A task's finish-no-earlier-than date.
    Fnet,
    /// A task's finish-no-later-than date.
    Fnlt,
    /// The date a started task's remaining work picks up again.
    Resume,
    /// A task's must-start-on date.
    StartPin,
    /// A task's must-finish-on date.
    FinishPin,
    /// A task's actual start.
    ActualStart,
    /// A task's actual finish.
    ActualFinish,
    /// The schedule's data date.
    DataDate,
    /// The schedule's must-finish-by date.
    FinishBy,
}

/// The task or dependency a refused value was given to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeItem {
    /// A task.
    Task(TaskId),
    /// A dependency; for a new one, the id it would have had.
    Dependency(DepId),
    /// The schedule itself, for an input no task or dependency owns.
    Schedule,
}

impl fmt::Display for RangeInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RangeInput::Duration => "duration",
            RangeInput::Lag => "lag",
            RangeInput::Remaining => "remaining duration",
            RangeInput::Snet => "start-no-earlier-than date",
            RangeInput::Snlt => "start-no-later-than date",
            RangeInput::Fnet => "finish-no-earlier-than date",
            RangeInput::Fnlt => "finish-no-later-than date",
            RangeInput::Resume => "resume date",
            RangeInput::StartPin => "start pin",
            RangeInput::FinishPin => "finish pin",
            RangeInput::ActualStart => "actual start",
            RangeInput::ActualFinish => "actual finish",
            RangeInput::DataDate => "data date",
            RangeInput::FinishBy => "must-finish-by date",
        })
    }
}

impl fmt::Display for RangeItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeItem::Task(t) => t.fmt(f),
            RangeItem::Dependency(d) => d.fmt(f),
            RangeItem::Schedule => f.write_str("the schedule"),
        }
    }
}

fn refuse(input: RangeInput, item: RangeItem, value: i64) -> ScheduleError {
    ScheduleError::OutOfRange { input, item, value }
}

/// A duration or remaining duration from 0 to [`MAX_DURATION`]: a negative one is
/// [`ScheduleError::NegativeDuration`].
pub(crate) fn check_duration(
    input: RangeInput,
    task: TaskId,
    duration: Minutes,
) -> Result<(), ScheduleError> {
    if duration < 0 {
        Err(ScheduleError::NegativeDuration(task))
    } else if duration > MAX_DURATION {
        Err(refuse(input, RangeItem::Task(task), duration))
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

/// A date, when one is set, from [`MIN_DATE`] to [`MAX_DATE`].
pub(crate) fn check_date(
    input: RangeInput,
    item: RangeItem,
    date: Option<Instant>,
) -> Result<(), ScheduleError> {
    match date {
        Some(d) if !(MIN_DATE..=MAX_DATE).contains(&d) => Err(refuse(input, item, d)),
        _ => Ok(()),
    }
}

/// Every date of a limit, its pin and its resume date included.
pub(crate) fn check_limit(task: TaskId, limit: &Limit) -> Result<(), ScheduleError> {
    let item = RangeItem::Task(task);
    check_date(RangeInput::Snlt, item, limit.snlt)?;
    check_date(RangeInput::Fnet, item, limit.fnet)?;
    check_date(RangeInput::Fnlt, item, limit.fnlt)?;
    check_date(RangeInput::Resume, item, limit.resume)?;
    check_date(RangeInput::StartPin, item, limit.start_pin(false))?;
    check_date(RangeInput::FinishPin, item, limit.finish_pin())
}

/// Every number a new task carries: its durations, dates and limits.
pub(crate) fn check_task(id: TaskId, task: &Task) -> Result<(), ScheduleError> {
    check_duration(RangeInput::Duration, id, task.duration)?;
    if let Some(r) = task.remaining {
        check_duration(RangeInput::Remaining, id, r)?;
    }
    let item = RangeItem::Task(id);
    check_date(RangeInput::Snet, item, task.snet)?;
    check_date(RangeInput::ActualStart, item, task.actual_start)?;
    check_date(RangeInput::ActualFinish, item, task.actual_finish)?;
    check_limit(id, &task.limit)
}
