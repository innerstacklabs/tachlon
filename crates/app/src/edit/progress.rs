//! The progress edit (gs#703): a task's actual start, actual finish and remaining duration are set
//! together, then a full solve, because an actual changes the task's late dates and its
//! predecessors' float as well as its early dates, which the cones do not re-derive. The cone is
//! a later optimisation.

use super::{Edit, EditError, parse_duration};
use crate::Session;
use engine::{Instant, Minutes, Schedule, ScheduleError, TaskId};

impl Session {
    pub(crate) fn run_set_progress(
        &mut self,
        task: TaskId,
        actual_start: Option<Instant>,
        actual_finish: Option<Instant>,
        remaining: Option<Minutes>,
    ) -> Result<(Edit, f64), EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        if self.outline.as_ref().is_some_and(|o| o.is_summary(task)) {
            let meta = &self.meta[task.0 as usize];
            return Err(EditError::ProgressOnSummary {
                name: meta.name.clone(),
            });
        }
        let before = &self.schedule.tasks()[task.0 as usize];
        let (was_start, was_finish, was_remaining) =
            (before.actual_start, before.actual_finish, before.remaining);
        let restore =
            |s: &mut Session| put(&mut s.schedule, task, was_start, was_finish, was_remaining);
        let apply = |s: &mut Schedule| {
            s.set_remaining(task, remaining)?;
            s.set_actual_start(task, actual_start)?;
            s.set_actual_finish(task, actual_finish)
        };
        if let Err(refused) = apply(&mut self.schedule) {
            restore(self);
            return Err(refused.into());
        }
        let elapsed = self.solve_full(Some(task), restore)?;
        let inverse = Edit::SetProgress {
            task,
            actual_start: was_start,
            actual_finish: was_finish,
            remaining: was_remaining,
        };
        Ok((inverse, elapsed))
    }

    /// Set a task's progress from table text: a day written `YYYY-MM-DD` for each actual and a
    /// duration (`3d`, `4h`) for the remaining time; empty text clears that field. One undo step.
    ///
    /// # Errors
    /// An [`EditError`] for text that is not a date or a duration, and for the refusals of
    /// `Edit::SetProgress`.
    pub fn set_progress_text(
        &mut self,
        task: TaskId,
        actual_start: &str,
        actual_finish: &str,
        remaining: &str,
    ) -> Result<(), EditError> {
        let edit = self.plan_progress_text(task, actual_start, actual_finish, remaining)?;
        self.edit(edit)
    }

    /// The edit [`Session::set_progress_text`] would run, without running it (for
    /// [`Session::edit_group`]).
    ///
    /// # Errors
    /// An [`EditError`] for an unknown task, bad text, and an actual finish before the start.
    pub fn plan_progress_text(
        &self,
        task: TaskId,
        actual_start: &str,
        actual_finish: &str,
        remaining: &str,
    ) -> Result<Edit, EditError> {
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let day = |text: &str| -> Result<Option<Instant>, EditError> {
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            self.origin
                .parse_day(text)
                .map(Some)
                .ok_or(EditError::BadDate)
        };
        let (actual_start, actual_finish) = (day(actual_start)?, day(actual_finish)?);
        let remaining = match remaining.trim() {
            "" => None,
            text => {
                let minutes = super::day_minutes(self.schedule.calendar_of(task));
                Some(parse_duration(text, minutes).map_err(EditError::Duration)?)
            }
        };
        let edit = Edit::SetProgress {
            task,
            actual_start,
            actual_finish,
            remaining,
        };
        self.check_actuals_not_after_data_date(task, actual_start, actual_finish)?;
        edit.check_typed().map(|()| edit)
    }

    /// Set one of a task's progress fields from the text its cell shows and keep the other two:
    /// `field` is `actual_start`, `actual_finish` or `remaining`. A date is written
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` (the table's own text), a remaining as a duration
    /// (`3d`, `4h`); empty text clears the field. One undo step. The page checks `field` against
    /// the three names before it calls; any other name is refused as a bad date.
    ///
    /// # Errors
    /// An [`EditError`] for an unknown `field`, text that is not a date or a duration, and the
    /// refusals of `Edit::SetProgress`.
    pub fn set_progress_field_text(
        &mut self,
        task: TaskId,
        field: &str,
        text: &str,
    ) -> Result<(), EditError> {
        let edit = self.plan_progress_field_text(task, field, text)?;
        self.edit(edit)
    }

    /// The edit [`Session::set_progress_field_text`] would run, without running it (for
    /// [`Session::edit_group`]).
    ///
    /// # Errors
    /// An [`EditError`] for an unknown task or `field`, bad text, and a finish before the start.
    pub fn plan_progress_field_text(
        &self,
        task: TaskId,
        field: &str,
        text: &str,
    ) -> Result<Edit, EditError> {
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let current = &self.schedule.tasks()[task.0 as usize];
        let (mut actual_start, mut actual_finish, mut remaining) = (
            current.actual_start,
            current.actual_finish,
            current.remaining,
        );
        let text = text.trim();
        match field {
            "actual_start" => actual_start = self.parse_moment(text)?,
            "actual_finish" => actual_finish = self.parse_moment(text)?,
            "remaining" => {
                remaining = if text.is_empty() {
                    None
                } else {
                    let minutes = super::day_minutes(self.schedule.calendar_of(task));
                    Some(parse_duration(text, minutes).map_err(EditError::Duration)?)
                };
            }
            _ => return Err(EditError::BadDate),
        }
        let edit = Edit::SetProgress {
            task,
            actual_start,
            actual_finish,
            remaining,
        };
        self.check_actuals_not_after_data_date(task, actual_start, actual_finish)?;
        edit.check_typed().map(|()| edit)
    }

    /// Refuse an actual start or finish typed later than the data date. An actual the task
    /// already holds passes, so remaining can still be edited on a task an import left after it.
    pub(crate) fn check_actuals_not_after_data_date(
        &self,
        task: TaskId,
        actual_start: Option<Instant>,
        actual_finish: Option<Instant>,
    ) -> Result<(), EditError> {
        let Some(data_date) = self.data_date() else {
            return Ok(());
        };
        let current = &self.schedule.tasks()[task.0 as usize];
        let typed = |new: Option<Instant>, old: Option<Instant>| {
            new.is_some_and(|date| date > data_date && new != old)
        };
        if typed(actual_start, current.actual_start) || typed(actual_finish, current.actual_finish)
        {
            return Err(EditError::ActualAfterDataDate);
        }
        Ok(())
    }

    /// An instant written `YYYY-MM-DD` (00:00) or `YYYY-MM-DD HH:MM`; empty text is none.
    fn parse_moment(&self, text: &str) -> Result<Option<Instant>, EditError> {
        if text.is_empty() {
            return Ok(None);
        }
        let (day, time) = text.split_once(' ').unwrap_or((text, "00:00"));
        let day = self.origin.parse_day(day).ok_or(EditError::BadDate)?;
        let digits = |part: &str| {
            part.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| part.parse::<i64>().ok())
                .flatten()
        };
        let (hour, minute) = time.split_once(':').ok_or(EditError::BadDate)?;
        match (digits(hour), digits(minute)) {
            (Some(h), Some(m)) if hour.len() == 2 && minute.len() == 2 && h < 24 && m < 60 => {
                Ok(Some(day + h * 60 + m))
            }
            _ => Err(EditError::BadDate),
        }
    }
}

/// Put a task's three progress fields back; the setters cannot refuse a live task and a value
/// they accepted before.
fn put(
    schedule: &mut Schedule,
    task: TaskId,
    start: Option<Instant>,
    finish: Option<Instant>,
    remaining: Option<Minutes>,
) {
    let _ = schedule.set_actual_start(task, start);
    let _ = schedule.set_actual_finish(task, finish);
    let _ = schedule.set_remaining(task, remaining);
}
