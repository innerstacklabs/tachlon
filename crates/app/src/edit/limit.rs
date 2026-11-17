//! The date-limit edit (gantt#1875): a full solve, since a limit changes late dates cones skip.

use super::{Edit, EditError};
use crate::Session;
use engine::{DAY, Limit, ScheduleError, TaskId};

impl Session {
    /// The day of limit `key` (`snlt`, `fnet` or `fnlt`) on task `i`, or an empty string when it
    /// is unset, the key is unknown, or the task is absent or deleted.
    pub fn task_limit_text(&self, i: u32, key: &str) -> String {
        let Some(limit) = self.live_task(i).map(|task| task.limit) else {
            return String::new();
        };
        let text = match key {
            "snlt" => limit.snlt.map(|t| self.origin.day_text(t)),
            "fnet" => limit.fnet.map(|t| self.origin.day_text(t - 1)),
            "fnlt" => limit.fnlt.map(|t| self.origin.day_text(t - 1)),
            _ => return String::new(),
        };
        text.unwrap_or_default()
    }

    /// Replace `task`'s limits, solve in full and return the edit that puts the old ones back.
    pub(crate) fn run_set_limit(
        &mut self,
        task: TaskId,
        limit: Limit,
    ) -> Result<(Edit, f64), EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let old = self.schedule.tasks()[task.0 as usize].limit;
        self.schedule.set_limit(task, limit)?;
        let elapsed = self.solve_full(Some(task), |s| {
            let _ = s.schedule.set_limit(task, old);
        })?;
        Ok((Edit::SetLimit { task, limit: old }, elapsed))
    }

    /// The edit that sets limit `key` (`snlt`, `fnet` or `fnlt`) of the live `task` to `text` (blank
    /// clears), keeping the other limits, the pin and as-late-as-possible. A finish is the day's end.
    pub(crate) fn plan_limit_text(
        &self,
        task: TaskId,
        key: &str,
        text: &str,
    ) -> Result<Edit, EditError> {
        let mut limit = self.schedule.tasks()[task.0 as usize].limit;
        let text = text.trim();
        let day_end = |text: &str| match text {
            "" => Ok(None),
            _ => self
                .origin
                .parse_day(text)
                .map(|day| Some(day.div_euclid(DAY) * DAY + DAY))
                .ok_or(EditError::BadDate),
        };
        match key {
            "snlt" => limit.snlt = self.parse_start(text).map_err(|_| EditError::BadDate)?,
            "fnet" => limit.fnet = day_end(text)?,
            "fnlt" => limit.fnlt = day_end(text)?,
            _ => return Err(EditError::BadDate),
        }
        Ok(Edit::SetLimit { task, limit })
    }
}
