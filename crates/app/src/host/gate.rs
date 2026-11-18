//! The edit gate (decision 0019, rule 3): one check in front of each action a person takes, never
//! in `run` or `apply`, so no drag frame pays for it. It holds read-only mode, then the host's
//! before-edit function; the capability profile joins them between the two (rule 5).

use super::planned::Plan;
use crate::{Session, edit::EditError};

impl Session {
    /// Read-only mode alone, ahead of an action's row checks; the host function is not asked.
    ///
    /// # Errors
    /// [`EditError::ReadOnly`] while the chart is read-only.
    pub(crate) fn refuse_read_only(&self) -> Result<(), EditError> {
        if self.host.read_only {
            return Err(EditError::ReadOnly);
        }
        Ok(())
    }

    /// Whether the person's action `plan` describes may go ahead.
    ///
    /// # Errors
    /// [`EditError::ReadOnly`], else [`EditError::NotAllowed`] for what the capability profile turned
    /// off, else the host function's [`EditError::Refused`] or [`EditError::BeforeEditFailed`].
    pub(crate) fn gate(&self, plan: &Plan) -> Result<(), EditError> {
        self.refuse_read_only()?;
        self.refuse_unallowed(plan)?;
        if let Some(before) = &self.host.before_edit {
            before(&self.planned(plan))?;
        }
        Ok(())
    }

    /// Switch read-only mode for the person using this chart. While it is on, every way to change
    /// the schedule is refused with [`EditError::ReadOnly`]; a click still selects a row. A press
    /// that has already passed the drag threshold finishes: the gate runs once per action, not per frame.
    pub fn set_read_only(&mut self, on: bool) {
        self.host.read_only = on;
    }

    /// Whether the chart is read-only.
    #[must_use]
    pub fn read_only(&self) -> bool {
        self.host.read_only
    }

    /// A press that passed the drag threshold is refused: the press is dropped, so no frame edits,
    /// and the drag note says why.
    pub(crate) fn refuse_drag(&mut self, error: &EditError) {
        self.press = None;
        let note = self.say(error);
        self.say_drag(note);
    }

    /// A press on a link terminal, or an Alt-press on a bar, is refused: it only selects `pred`,
    /// and the drag note says why no link starts.
    pub(crate) fn refuse_link(&mut self, pred: u32, error: &EditError) {
        let note = self.say(error);
        self.say_drag(note);
        self.select(pred);
    }
}
