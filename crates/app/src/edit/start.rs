//! Typing a task's Start cell (gs#703): the text becomes the task's start-no-earlier-than through
//! [`Edit::Move`], the edit a drag uses, as one undo step. The edit runs when a cell is committed,
//! never per frame.

use super::{Edit, EditError};
use crate::Session;
use crate::texts::{
    START_ALREADY_LATER, START_BAD_DATE, START_NOTHING_TO_REMOVE, START_SUMMARY, START_UNCHANGED,
    Texts, Worded, english_display,
};
use engine::{Instant, ScheduleError, TaskId};

/// What a typed start did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartOutcome {
    /// The constraint changed: one undo step.
    Set,
    /// The links already start the task at or after the typed date: nothing changed.
    AlreadyLater,
    /// The task already has this constraint: nothing changed.
    Unchanged,
    /// Empty text, and the task has no start constraint (none, or only its import floor).
    NothingToRemove,
}

/// Why a typed start was refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// A summary's dates are its sub-tasks'.
    Summary { name: String },
    /// The text is not `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`.
    BadDate,
    /// The edit itself was refused (a started task, a bar press, an unknown task).
    Edit(EditError),
}

impl std::error::Error for StartError {}
impl Worded for StartError {
    fn text(&self, texts: &Texts) -> String {
        match self {
            Self::Summary { name } => texts.say(&START_SUMMARY, &[("name", name)]),
            Self::BadDate => texts.say(&START_BAD_DATE, &[]),
            Self::Edit(error) => error.text(texts),
        }
    }
}
english_display!(StartError);

impl StartOutcome {
    /// What the status line says when nothing changed; none after a change.
    #[must_use]
    pub fn note(self, texts: &Texts) -> Option<String> {
        let text = match self {
            Self::Set => return None,
            Self::AlreadyLater => &START_ALREADY_LATER,
            Self::Unchanged => &START_UNCHANGED,
            Self::NothingToRemove => &START_NOTHING_TO_REMOVE,
        };
        Some(texts.say(text, &[]))
    }
}

impl Session {
    /// Set task `task`'s start-no-earlier-than from the text its Start cell takes: `YYYY-MM-DD`
    /// (00:00) or `YYYY-MM-DD HH:MM`; empty text removes the constraint, back to the task's import
    /// floor when it has one. One undo step.
    /// A summary, a task with an actual start or finish, an active bar press and text that is not
    /// a date are refused, changing nothing. A date the links already pass changes nothing.
    ///
    /// # Errors
    /// [`StartError::Summary`] for a summary, [`StartError::BadDate`] for text that is not a date,
    /// else [`StartError::Edit`].
    pub fn set_start_text(&mut self, task: TaskId, text: &str) -> Result<StartOutcome, StartError> {
        let (outcome, edit) = self.plan_start_text(task, text)?;
        if let Some(edit) = edit {
            self.edit(edit).map_err(StartError::Edit)?;
        }
        Ok(outcome)
    }

    /// What [`Session::set_start_text`] would do with `text`, without doing it: the outcome and, for
    /// [`StartOutcome::Set`], the edit to run (for [`Session::edit`] or [`Session::edit_group`]);
    /// `None` when nothing would change.
    ///
    /// # Errors
    /// The same as [`Session::set_start_text`], except the refusals of the edit itself.
    pub fn plan_start_text(
        &self,
        task: TaskId,
        text: &str,
    ) -> Result<(StartOutcome, Option<Edit>), StartError> {
        if !self.is_live(task) {
            return Err(StartError::Edit(ScheduleError::UnknownTask(task).into()));
        }
        if let Some(outline) = self.outline.as_ref()
            && outline.is_summary(task)
        {
            let name = self.meta[task.0 as usize].name.clone();
            return Err(StartError::Summary { name });
        }
        let wanted = self.parse_start(text.trim())?;
        let current = &self.schedule.tasks()[task.0 as usize];
        if current.actual_start.is_some() || current.actual_finish.is_some() {
            return Err(StartError::Edit(EditError::HasActuals));
        }
        // Clearing puts an imported task back at its floor, which is no constraint (K2).
        let wanted = if wanted.is_none() {
            if self.start_constraint(task).is_none() {
                return Ok((StartOutcome::NothingToRemove, None));
            }
            self.meta[task.0 as usize].floor
        } else {
            wanted
        };
        if current.snet == wanted {
            return Ok((StartOutcome::Unchanged, None));
        }
        if current.snet.is_none() && self.links_pass(task, wanted) {
            return Ok((StartOutcome::AlreadyLater, None));
        }
        Ok((StartOutcome::Set, Some(Edit::Move { task, snet: wanted })))
    }

    /// Whether the task would start where it does with the constraint `wanted` added.
    fn links_pass(&self, task: TaskId, wanted: Option<Instant>) -> bool {
        let mut trial = self.schedule.clone();
        trial.set_snet(task, wanted).is_ok()
            && trial.compute().is_ok_and(|solution| {
                solution.dates[task.0 as usize].es == self.solution.dates[task.0 as usize].es
            })
    }

    /// An instant written `YYYY-MM-DD` (00:00) or `YYYY-MM-DD HH:MM`; empty text is none.
    pub(crate) fn parse_start(&self, text: &str) -> Result<Option<Instant>, StartError> {
        if text.is_empty() {
            return Ok(None);
        }
        let (day, time) = text.split_once(' ').unwrap_or((text, "00:00"));
        let day = self.origin.parse_day(day).ok_or(StartError::BadDate)?;
        let (hour, minute) = time.split_once(':').ok_or(StartError::BadDate)?;
        let digits = |part: &str| {
            (part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<i64>().ok())
                .flatten()
        };
        match (digits(hour), digits(minute)) {
            (Some(h), Some(m)) if h < 24 && m < 60 => Ok(Some(day + h * 60 + m)),
            _ => Err(StartError::BadDate),
        }
    }
}
