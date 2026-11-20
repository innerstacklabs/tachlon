//! Typing a task's Finish cell (gs#703): the start stays and the duration becomes the working time
//! from it to the end of the typed day, through [`Edit::SetDuration`], which is its own inverse.

use super::{Edit, EditError};
use crate::Session;
use crate::texts::{FINISH_DAY_BEFORE_START, FINISH_SUMMARY, Texts, Worded, english_display};
use engine::{ScheduleError, TaskId};

const DAY: i64 = 24 * 60;

/// Why a typed finish was refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishError {
    /// A summary's dates are its sub-tasks'.
    Summary { name: String },
    /// The day is before the day the task starts (`start`, as `YYYY-MM-DD`).
    BeforeStart { start: String },
    /// The text is not a day, or the edit itself was refused (a bar press, an unknown task).
    Edit(EditError),
}

impl std::error::Error for FinishError {}
impl Worded for FinishError {
    fn text(&self, texts: &Texts) -> String {
        match self {
            Self::Summary { name } => texts.say(&FINISH_SUMMARY, &[("name", name)]),
            Self::BeforeStart { start } => texts.say(&FINISH_DAY_BEFORE_START, &[("start", start)]),
            Self::Edit(error) => error.text(texts),
        }
    }
}
english_display!(FinishError);

impl Session {
    /// The duration that ends `task` at the end of the day that holds instant `at`: the working
    /// time of its calendar from its start to then. `None` when that day ends at or before the
    /// start. The typed Finish cell and the drag of a bar's finish edge share it.
    pub(crate) fn duration_to_day_end(&self, task: TaskId, at: i64) -> Option<i64> {
        self.duration_from(task, self.solution.dates[task.0 as usize].es, at)
    }

    /// [`Session::duration_to_day_end`] from the instant `start`, not the start `task` has now.
    fn duration_from(&self, task: TaskId, start: i64, at: i64) -> Option<i64> {
        let end = at.div_euclid(DAY) * DAY + DAY;
        (end > start).then(|| self.schedule.calendar_of(task).working_between(start, end))
    }

    /// Set task `task`'s duration from the day its Finish cell takes, `YYYY-MM-DD`: the working time
    /// of its calendar from its start to the end of that day (a Saturday after a Friday start ends
    /// on the Friday). One undo step; a day that gives the duration it has changes nothing.
    ///
    /// # Errors
    /// [`FinishError::Summary`] for a summary, [`FinishError::BeforeStart`] for a day before the
    /// start's, else [`FinishError::Edit`] (text that is not a day, a task with an actual start or
    /// finish, an unknown task, a bar press).
    pub fn set_finish_text(&mut self, task: TaskId, text: &str) -> Result<(), FinishError> {
        match self.plan_finish_text(task, text)? {
            Some(edit) => self.edit(edit).map_err(FinishError::Edit),
            None => Ok(()),
        }
    }

    /// The edit [`Session::set_finish_text`] would run for `text`, without running it (for
    /// [`Session::edit_group`]); `None` when the day gives the duration the task has.
    ///
    /// # Errors
    /// The same as [`Session::set_finish_text`], except the refusals of the edit itself.
    pub fn plan_finish_text(&self, task: TaskId, text: &str) -> Result<Option<Edit>, FinishError> {
        self.plan_finish_from(task, text, None)
    }

    /// [`Session::plan_finish_text`] measured from `start` when given: the start a dialog's own
    /// start edit leaves.
    pub(crate) fn plan_finish_from(
        &self,
        task: TaskId,
        text: &str,
        start: Option<i64>,
    ) -> Result<Option<Edit>, FinishError> {
        if !self.is_live(task) {
            return Err(FinishError::Edit(ScheduleError::UnknownTask(task).into()));
        }
        if let Some(outline) = self.outline.as_ref()
            && outline.is_summary(task)
        {
            return Err(FinishError::Summary {
                name: self.meta[task.0 as usize].name.clone(),
            });
        }
        let day = self
            .origin
            .parse_day(text.trim())
            .ok_or(FinishError::Edit(EditError::BadDate))?;
        let current = &self.schedule.tasks()[task.0 as usize];
        if current.actual_start.is_some() || current.actual_finish.is_some() {
            return Err(FinishError::Edit(EditError::HasActuals));
        }
        let start = start.unwrap_or(self.solution.dates[task.0 as usize].es);
        let Some(minutes) = self.duration_from(task, start, day) else {
            return Err(FinishError::BeforeStart {
                start: self.date_text(start).chars().take(10).collect(),
            });
        };
        if minutes == self.schedule.tasks()[task.0 as usize].duration {
            return Ok(None);
        }
        Ok(Some(Edit::SetDuration { task, minutes }))
    }
}
