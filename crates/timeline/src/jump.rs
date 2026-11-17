//! Jumping the view to the next critical task.

use engine::TaskDates;

use crate::Timeline;

impl Timeline {
    /// Scroll so the next critical task at or after the current top row
    /// (wrapping to the first) sits at the top, and move time to its start.
    /// Returns its row, or None if no task is critical.
    pub fn jump_to_next_critical(&mut self, dates: &[TaskDates]) -> Option<usize> {
        self.jump_to_next_critical_where(dates, |_| true)
    }

    /// Scroll to the next current-row task that is critical and accepted by
    /// `keep`, using the same cycling behavior as [`Self::jump_to_next_critical`].
    pub fn jump_to_next_critical_where(
        &mut self,
        dates: &[TaskDates],
        keep: impl Fn(u32) -> bool,
    ) -> Option<usize> {
        let n = self.rows().len();
        if n == 0 {
            return None;
        }
        // Continue from the last jump while the view still shows it, so a
        // clamped scroll near the end cannot repeat the same row forever.
        // Otherwise start from the row before the top row, so the top row is
        // the first candidate and (at row 0, by wrapping) not the last.
        let rows = self.viewport.visible_rows();
        let from = match self.last_jump {
            Some(r) if rows.contains(&r) => r,
            _ => (rows.start.min(n - 1) + n - 1) % n,
        };
        let row = (1..=n).map(|i| (from + i) % n).find(|&r| {
            let task = self.rows()[r];
            task != crate::HIDDEN_ROW
                && self.summaries.get(task).is_none()
                && dates[task as usize].is_critical()
                && keep(task)
        })?;
        self.last_jump = Some(row);
        self.viewport.scroll_to_row(row);
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
        )]
        let es = dates[self.rows()[row] as usize].es as f64;
        let x = self.viewport.x_of(es);
        // Put the bar's start a fifth of the way across.
        self.viewport.pan(self.viewport.width * 0.2 - x, 0.0);
        Some(row)
    }
}
