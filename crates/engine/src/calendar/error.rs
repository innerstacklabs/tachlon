//! Why a calendar could not be built.

use std::fmt;

use super::{DayPeriod, MAX_EXCEPTION_DAY};

/// Why a calendar could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarError {
    /// A day's periods were out of range, empty, unsorted or overlapping.
    BadPeriod {
        /// Weekday, 0 = Monday.
        weekday: usize,
        /// The offending period.
        period: DayPeriod,
    },
    /// An exception day's periods were out of range, empty, unsorted or
    /// overlapping.
    BadException {
        /// Day index of the exception.
        day: i64,
        /// The offending period.
        period: DayPeriod,
    },
    /// The weekly pattern has no working minute, so `add` could never finish.
    /// Exceptions cannot rescue it: `add` must be able to finish past the
    /// last exception.
    NoWorkingTime,
    /// An exception day (a holiday included) before the origin.
    NegativeException(i64),
    /// Two exceptions on the same day.
    DuplicateException(i64),
    /// An exception day (a holiday included) after [`MAX_EXCEPTION_DAY`],
    /// whose end does not fit in an [`Instant`](super::Instant).
    ExceptionTooLate(i64),
}

impl fmt::Display for CalendarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalendarError::BadPeriod { weekday, period } => write!(
                f,
                "weekday {weekday}: period {}..{} is empty, out of 0..=1440, unsorted or overlapping",
                period.0, period.1
            ),
            CalendarError::BadException { day, period } => write!(
                f,
                "exception day {day}: period {}..{} is empty, out of 0..=1440, unsorted or overlapping",
                period.0, period.1
            ),
            CalendarError::NoWorkingTime => write!(f, "calendar has no working minute in its week"),
            CalendarError::NegativeException(d) => {
                write!(f, "exception day {d} is before the origin")
            }
            CalendarError::DuplicateException(d) => {
                write!(f, "exception day {d} is given more than once")
            }
            CalendarError::ExceptionTooLate(d) => write!(
                f,
                "exception day {d} is after the last supported day {MAX_EXCEPTION_DAY}"
            ),
        }
    }
}

impl std::error::Error for CalendarError {}
