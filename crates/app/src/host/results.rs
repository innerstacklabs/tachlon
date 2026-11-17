//! Every task's dates and float, listed for a program (gantt#2081), in one read where the hover card
//! gives one task at a time. The session is read, never changed.

use crate::{Session, json::quote};
use engine::{Instant, Minutes, ScheduleError, TaskId};

/// `minutes` as days of `day` minutes: the exact quotient, so it can be a fraction.
#[expect(clippy::cast_precision_loss, reason = "minutes are far below 2^53")]
fn days(minutes: Minutes, day: Minutes) -> f64 {
    minutes as f64 / day as f64
}

impl Session {
    /// Every live task that is not a summary, in slot order, as a JSON array of `{"task","key","code",
    /// "start","finish","lateStart","lateFinish","float","freeFloat","critical","finished"}`: `task` is
    /// the session slot, `key` and `code` are text or `null`, the dates are `YYYY-MM-DDTHH:mm`, and the
    /// floats are working days of the task's calendar (decision 0017), negative past a late limit.
    ///
    /// # Errors
    ///
    /// The engine's error when the free floats cannot be worked out.
    pub fn results_json(&self) -> Result<String, ScheduleError> {
        let (schedule, solution) = (self.schedule(), self.solution());
        let free = schedule.free_floats(solution)?;
        let date = |t: Instant| quote(&self.date_text(t).replacen(' ', "T", 1));
        let text =
            |value: &Option<String>| value.as_deref().map_or_else(|| "null".to_owned(), quote);
        let rows: Vec<String> = (0..)
            .zip(&solution.dates)
            .filter(|&(n, _)| {
                !schedule.tasks()[n as usize].deleted
                    && !self.outline().is_some_and(|outline| outline.is_summary(TaskId(n)))
            })
            .map(|(n, dates)| {
                let meta = &self.meta()[n as usize];
                let day = crate::edit::day_minutes(schedule.calendar_of(TaskId(n)));
                format!(
                    r#"{{"task":{n},"key":{},"code":{},"start":{},"finish":{},"lateStart":{},"lateFinish":{},"float":{},"freeFloat":{},"critical":{},"finished":{}}}"#,
                    text(&meta.key),
                    text(&meta.code),
                    date(dates.es),
                    date(dates.ef),
                    date(schedule.late_start_shown(TaskId(n), dates)),
                    date(dates.lf),
                    days(dates.float, day),
                    days(free[n as usize], day),
                    dates.is_critical(),
                    dates.finished,
                )
            })
            .collect();
        Ok(format!("[{}]", rows.join(",")))
    }
}
