//! A host's values as the session keeps them (decision 0017): a date is read by the kind of its
//! field, a duration as whole working minutes, the engine's own unit.

use crate::{Session, document::json_read::Value, edit::day_minutes, json::quote};
use engine::{DAY, DepId, Instant, LagCalendar, Minutes, TaskId};

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
}

impl Session {
    /// The working day in minutes of every task's calendar and of every link's lag calendar, by slot, as `{"tasks":[..],"links":[..]}`, what a duration or a lag in days is a multiple of; with the host key of each task slot (`keys`, `null` for none) and the schedule's lag calendar (`lag`: `"successor"`, `"predecessor"` or its day), which find the day of a link not made yet.
    #[must_use]
    pub fn day_lengths(&self) -> String {
        let schedule = self.schedule();
        let days = |task: u32| day_minutes(schedule.calendar_of(TaskId(task)));
        let tasks: Vec<Minutes> = (0..)
            .zip(schedule.tasks())
            .map(|(at, _)| days(at))
            .collect();
        let links: Vec<Minutes> = (0..)
            .zip(schedule.dependencies())
            .map(|(at, _)| self.lag_day_minutes(DepId(at)))
            .collect();
        let keys: Vec<String> = (self.meta.iter())
            .map(|m| m.key.as_deref().map_or("null".into(), quote))
            .collect();
        let lag: String = match schedule.lag_calendar() {
            LagCalendar::Successor => r#""successor""#.into(),
            LagCalendar::Predecessor => r#""predecessor""#.into(),
            LagCalendar::TwentyFourHour => DAY.to_string(),
            LagCalendar::Fixed(id) => day_minutes(&schedule.calendars()[id.0 as usize]).to_string(),
        };
        let keys = keys.join(",");
        format!(r#"{{"tasks":{tasks:?},"links":{links:?},"keys":[{keys}],"lag":{lag}}}"#)
    }
}

/// The whole, non-negative number of working minutes `value` is, or `None` (a fractional number too).
pub(super) fn read_duration(value: &Value) -> Option<Minutes> {
    value.as_int().filter(|&n| n >= 0)
}
