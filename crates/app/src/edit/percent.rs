//! Typing a percent complete (gs#703): a task stores its remaining duration and derives the
//! percent (decision 0008, pick 1), so the typed percent becomes the remaining duration (rounded to whole working hours), together
//! with the actual start and actual finish it implies, as one `Edit::SetProgress` whose inverse
//! puts all three fields back.

use super::{Edit, EditError};
use crate::Session;
use engine::{Instant, Minutes, ScheduleError, TaskDates, TaskId};

impl Session {
    /// Set a task's percent complete from the text its cell shows: `75` or `75%`, a whole number
    /// from 0 to 100. The task keeps that share of its duration as done and the rest as
    /// remaining; a task with no actual start takes its early start as one, except for `0`, which
    /// leaves it unstarted. `100` finishes the task at its early finish (or at its actual start,
    /// if that is later); any other value clears the actual finish. One undo step. Text that is
    /// not a percent from 0 to 100, a summary and an actual date after the data date are refused,
    /// changing nothing.
    ///
    /// # Errors
    /// An [`EditError`] for an unknown or deleted task, a summary, text that is not a percent, or
    /// an actual date after the data date.
    pub fn set_percent_text(&mut self, task: TaskId, text: &str) -> Result<(), EditError> {
        let edit = self.plan_percent_text(task, text)?;
        self.edit(edit)?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "parse_percent accepts only digits and at most 100, so percent is 0 to 100"
        )]
        self.typed_percent
            .insert(task.0, parse_percent(text)? as u8);
        Ok(())
    }

    /// The edit [`Session::set_percent_text`] would run, without running it (for
    /// [`Session::edit_group`]); the setter also records the typed percent, which a caller that
    /// runs the edit itself must do too.
    ///
    /// # Errors
    /// The same as [`Session::set_percent_text`], except the refusals of the edit itself.
    pub fn plan_percent_text(&self, task: TaskId, text: &str) -> Result<Edit, EditError> {
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let percent = parse_percent(text)?;
        let duration = self.schedule.tasks()[task.0 as usize].duration;
        let (actual_start, actual_finish, remaining) =
            self.percent_progress(&self.solution.dates, task, percent, duration);
        self.check_actuals_not_after_data_date(task, actual_start, actual_finish)?;
        Ok(Edit::SetProgress {
            task,
            actual_start,
            actual_finish,
            remaining,
        })
    }

    /// The actual start, actual finish and remaining work that `percent` of `duration` sets on a live
    /// task, from the early dates `dates` holds. `0` on a task with no actual start sets none.
    pub(crate) fn percent_progress(
        &self,
        dates: &[TaskDates],
        task: TaskId,
        percent: i64,
        duration: Minutes,
    ) -> (Option<Instant>, Option<Instant>, Option<Minutes>) {
        let held = self.schedule.tasks()[task.0 as usize].actual_start;
        if percent == 0 && held.is_none() {
            return (None, None, None);
        }
        let actual_start = held.unwrap_or_else(|| dates[task.0 as usize].es);
        let (actual_finish, remaining) = if percent == 100 {
            let finish: Instant = dates[task.0 as usize].ef.max(actual_start);
            (Some(finish), 0)
        } else {
            (None, whole_hours_left(duration, percent))
        };
        (Some(actual_start), actual_finish, Some(remaining))
    }
}

impl Session {
    /// The percent a person typed for task `i`, while it still holds: the task is not finished and
    /// its remaining is what that percent set. Any other edit of the remaining work ends it, and
    /// the cell goes back to the percent derived from the remaining work.
    pub(crate) fn typed_percent_shown(&self, i: u32) -> Option<u8> {
        let typed = *self.typed_percent.get(&i)?;
        let task = self.schedule.tasks().get(i as usize)?;
        (task.actual_finish.is_none()
            && task.remaining == Some(whole_hours_left(task.duration, i64::from(typed))))
        .then_some(typed)
    }
}

/// The share of `duration` still to do, in whole working hours: the nearest hour, halves up, and
/// never 0 for a percent below 100, so a finish and the successor's start land on the hour.
#[expect(
    clippy::cast_possible_truncation,
    reason = "hours * 60 is at most duration + 30 minutes (60 when duration is 0), so it fits i64 unless duration is within 30 of i64::MAX; nothing here caps duration, and past that it wraps"
)]
fn whole_hours_left(duration: i64, percent: i64) -> i64 {
    let left = i128::from(duration) * i128::from(100 - percent);
    let hours = (left * 2 + 6_000) / 12_000;
    (hours.max(1) * 60) as i64
}

/// A whole number from 0 to 100, with an optional `%`.
fn parse_percent(text: &str) -> Result<i64, EditError> {
    let text = text.trim();
    let digits = text.strip_suffix('%').unwrap_or(text).trim_end();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(EditError::BadPercent);
    }
    match digits.parse::<i64>() {
        Ok(percent) if percent <= 100 => Ok(percent),
        _ => Err(EditError::BadPercent),
    }
}
