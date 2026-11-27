//! Typing a physical percent complete (tl#2080, decision 0008 item 1a): it is stored in the task's
//! metadata apart from the percent derived from the remaining work, so setting it moves no date
//! and runs no solve.

use super::{Edit, EditError};
use crate::Session;
use engine::{ScheduleError, TaskId};

impl Session {
    /// Set or clear a task's physical percent from the text its cell shows: `50`, `50%`, `12.5` or
    /// `12.35%`, from 0 to 100 with up to two decimals; empty text clears it. One undo step that
    /// puts the old value back. A value the task already holds makes no edit and no undo step.
    /// A summary and text that is not such a percent are refused, changing nothing.
    ///
    /// # Errors
    /// An [`EditError`] for an unknown or deleted task, a summary, or text that is not a percent.
    pub fn set_physical_text(&mut self, task: TaskId, text: &str) -> Result<(), EditError> {
        let physical = parse_physical(text)?;
        self.edit(Edit::SetPhysical { task, physical })
    }

    /// Whether `edit` sets the physical percent a live task that is no summary already holds:
    /// [`Session::edit`] makes no edit of it, after its gate, so no undo step undoes nothing.
    pub(crate) fn is_unchanged_physical(&self, edit: &Edit) -> bool {
        let Edit::SetPhysical { task, physical } = edit else {
            return false;
        };
        let summary = self.outline.as_ref().is_some_and(|o| o.is_summary(*task));
        let held = self.meta.get(task.0 as usize).map(|meta| meta.physical);
        self.is_live(*task) && !summary && held == Some(*physical)
    }

    pub(crate) fn run_set_physical(
        &mut self,
        task: TaskId,
        physical: Option<u16>,
    ) -> Result<(Edit, f64), EditError> {
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        if self.outline.as_ref().is_some_and(|o| o.is_summary(task)) {
            let name = self.meta[task.0 as usize].name.clone();
            return Err(EditError::ProgressOnSummary { name });
        }
        let was = std::mem::replace(&mut self.meta[task.0 as usize].physical, physical);
        self.changed();
        Ok((
            Edit::SetPhysical {
                task,
                physical: was,
            },
            0.0,
        ))
    }
}

/// A physical percent as a plain number for a host and a cell (`50`, `12.5`, `12.35`): hundredths
/// of a percent without trailing zeros.
pub(crate) fn percent_number(hundredths: u16) -> String {
    let text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `None` for empty text, else hundredths of a percent from a number from 0 to 100 with up to two
/// decimals and an optional `%`.
fn parse_physical(text: &str) -> Result<Option<u16>, EditError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let number = text.strip_suffix('%').unwrap_or(text).trim_end();
    let (whole, decimals) = number.split_once('.').unwrap_or((number, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let trailing_dot = decimals.is_empty() && number.contains('.');
    if whole.is_empty() || decimals.len() > 2 || trailing_dot || !digits(whole) || !digits(decimals)
    {
        return Err(EditError::BadPhysical);
    }
    let whole = whole.parse::<u32>().ok().filter(|&w| w <= 100);
    let cents = format!("{decimals:0<2}").parse::<u32>().ok();
    whole
        .zip(cents)
        .map(|(whole, cents)| whole * 100 + cents)
        .and_then(|total| u16::try_from(total).ok().filter(|&t| t <= 10_000))
        .map(Some)
        .ok_or(EditError::BadPhysical)
}
