//! A host's values as the session keeps them (decision 0017): a date is read by the kind of its
//! field, a duration as whole working minutes, the engine's own unit.

use crate::{Session, document::json_read::Value};
use engine::{DAY, Instant, Minutes, TaskId};

impl Session {
    /// The start-kind date `value` holds, `YYYY-MM-DD` or `YYYY-MM-DDTHH:mm`, or `None` for anything else.
    pub(super) fn read_date(&self, value: &Value) -> Option<Instant> {
        let text = value.as_str().filter(|text| !text.contains(' '))?;
        self.parse_start(&text.replacen('T', " ", 1)).ok()?
    }

    /// The finish-kind date `value` holds: a date-only text is the end of that day, a text with a time that minute.
    pub(super) fn read_finish(&self, value: &Value) -> Option<Instant> {
        let at = self.read_date(value)?;
        Some(if value.as_str()?.contains('T') {
            at
        } else {
            at + DAY
        })
    }

    /// What a date alone names on `task`'s calendar (decision 0017, rule 6), for the day that starts at `day`: the
    /// first working minute for a start, the end of the last working minute for a `finish`. A day with no working
    /// time gives `day` for a start and 24:00 for a finish.
    pub(crate) fn day_edge(&self, task: TaskId, day: Instant, finish: bool) -> Instant {
        let calendar = self.schedule.calendar_of(task);
        if finish {
            (calendar.prev_period_end(day + DAY))
                .filter(|&at| at > day)
                .unwrap_or(day + DAY)
        } else {
            Some(calendar.next_working(day))
                .filter(|&at| at < day + DAY)
                .unwrap_or(day)
        }
    }

    /// The date of a limit of `task` that `value` holds: a date alone is read by [`Session::day_edge`], a text with a
    /// time is that minute.
    pub(super) fn read_limit(&self, task: TaskId, value: &Value, finish: bool) -> Option<Instant> {
        let at = self.read_date(value)?;
        let timed = value.as_str()?.contains('T');
        Some(if timed {
            at
        } else {
            self.day_edge(task, at, finish)
        })
    }
}

/// The whole, non-negative number of working minutes `value` is, or `None` (a fractional number too).
pub(super) fn read_duration(value: &Value) -> Option<Minutes> {
    value.as_int().filter(|&n| n >= 0)
}
