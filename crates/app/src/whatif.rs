//! A what-if mark (gantt#703): the history depth and the dates at the start; discarding is `undo`.

use crate::Session;
use crate::baseline::Baseline;
use crate::edit::EditError;
use crate::host::Plan;

/// The history's mark; `depth` is `None` once the history dropped steps before it.
pub(crate) struct Mark {
    pub(crate) depth: Option<usize>,
    start: Baseline,
}

/// Why a what-if call was refused: a press is in progress, one is on already, none is on, the
/// history dropped the start (keep it instead), or an undo step was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhatIfError {
    Busy,
    AlreadyOn,
    NotOn,
    Lost,
    Undo(EditError),
}

/// How the schedule differs from the mark: dates as the toolbar writes them (`14 Oct`), the
/// finish change in calendar days, and the edits still applied since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatIfSummary {
    pub finish_was: String,
    pub finish_now: String,
    pub finish_days: i64,
    pub slipped: u32,
    pub joined: u32,
    pub left: u32,
    pub edits: usize,
}

impl Session {
    /// Mark the schedule as it is.
    /// # Errors
    /// `Busy` during a press; `AlreadyOn` with a mark on.
    pub fn begin_whatif(&mut self) -> Result<(), WhatIfError> {
        if self.busy() {
            return Err(WhatIfError::Busy);
        }
        if self.history.whatif.is_some() {
            return Err(WhatIfError::AlreadyOn);
        }
        let start = Baseline::snapshot(&self.schedule, &self.solution, self.data_date);
        let depth = Some(self.history_len().0);
        self.history.whatif = Some(Mark { depth, start });
        self.stamp = self.stamp.wrapping_add(1);
        Ok(())
    }

    /// How the schedule differs from the mark, or `None` with none on.
    pub fn whatif_summary(&self) -> Option<WhatIfSummary> {
        let mark = self.history.whatif.as_ref()?;
        let sum = self.summarize(&mark.start);
        Some(WhatIfSummary {
            finish_was: self.day_month(mark.start.project_finish()),
            finish_now: self.day_month(self.solution.project_finish),
            finish_days: sum.finish_days,
            slipped: sum.slipped,
            joined: sum.joined,
            left: sum.left,
            edits: self.history_len().0 - mark.depth.unwrap_or(0),
        })
    }

    /// End the what-if and keep the edits.
    /// # Errors
    /// `NotOn` with none on.
    pub fn keep_whatif(&mut self) -> Result<(), WhatIfError> {
        self.history.whatif.take().ok_or(WhatIfError::NotOn)?;
        self.stamp = self.stamp.wrapping_add(1);
        Ok(())
    }

    /// Undo every edit since the mark and end it; the undone edits stay on the redo stack.
    /// # Errors
    /// `NotOn`; `Busy` during a press; `Lost` when the history dropped the start; `Undo` when a
    /// step is refused.
    pub fn discard_whatif(&mut self) -> Result<(), WhatIfError> {
        let mark = self.history.whatif.as_ref().ok_or(WhatIfError::NotOn)?;
        let depth = mark.depth.ok_or(WhatIfError::Lost)?;
        self.gate(&Plan::Discard(depth))
            .map_err(WhatIfError::Undo)?;
        while self.history_len().0 > depth {
            if !self.undo_step().map_err(WhatIfError::Undo)? {
                return Err(WhatIfError::Busy);
            }
        }
        self.keep_whatif()
    }
}
