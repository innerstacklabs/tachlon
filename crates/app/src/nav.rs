//! Cell cursor rows (gs#416, story S1 of #300): the cursor's row is the selected task, and these
//! two calls move it. The cursor's column is the page's (`packages/gantt/src/cursor.ts`).

use crate::Session;
use engine::TaskId;

impl Session {
    /// Select `task` from a click on its table row. False, changing nothing, while a bar press or
    /// a drag is in progress, and for a task that is deleted, unknown or has no row (hidden by a
    /// collapsed summary or the row filter).
    pub fn select_task(&mut self, task: u32) -> bool {
        if self.busy() || !self.is_live(TaskId(task)) || self.timeline().row_of_task(task).is_none()
        {
            return false;
        }
        self.select(task);
        self.pan_to_bar_start(task);
        true
    }

    /// Move the selection `delta` visible rows down (negative: up), stopping at the first or last
    /// row. The rows are the table's own, so a collapsed summary's children and the rows a filter
    /// hides are skipped. With nothing selected, the first row is selected. A selection hidden
    /// under a collapsed summary moves from that summary's row. Returns the selected task, or
    /// `None`, changing nothing, while a bar press or a drag is in progress or when there are no
    /// rows.
    pub fn move_selection(&mut self, delta: i32) -> Option<u32> {
        if self.busy() {
            return None;
        }
        let target = self.timeline().step_task_row(self.cursor_row(), delta)?;
        let task = self.timeline().task_at_row(target)?;
        if self.selection().map(|selected| selected.task) != Some(task) {
            self.select(task);
            self.pan_to_bar_start(task);
        }
        Some(task)
    }

    /// Pans, as Find does, so `task`'s bar start sits a fifth of the way in from the left, when
    /// that start is outside the canvas. A bar that starts on screen does not move the view, and
    /// the zoom never changes. A pan ends the opening fit.
    fn pan_to_bar_start(&mut self, task: u32) {
        let start = self
            .timeline()
            .summaries()
            .get(task)
            .map(|span| span.0)
            .or_else(|| {
                self.solution()
                    .dates
                    .get(task as usize)
                    .map(|dates| dates.es)
            });
        let Some(start) = start else { return };
        let viewport = &self.timeline().viewport;
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        let (width, x) = (viewport.width, viewport.x_of(start as f64));
        if !(0.0..=width).contains(&x) {
            self.fit_pending = false;
            self.timeline.viewport.pan(width * 0.2 - x, 0.0);
        }
    }

    /// The row the cursor is on: the selected task's row, else its nearest ancestor that has one.
    fn cursor_row(&self) -> Option<usize> {
        self.visible_row_of(self.selection()?.task)
    }

    /// The row of `task`, else of its nearest ancestor that has one: a task hidden under a
    /// collapsed summary stands on that summary's row, and while grouped, one hidden under a closed
    /// group stands on that group's header.
    pub(crate) fn visible_row_of(&self, task: u32) -> Option<usize> {
        let own = self.timeline().row_of_task(task);
        if let Some(row) = own.or_else(|| self.group_header_row(task)) {
            return Some(row);
        }
        let mut task = TaskId(task);
        loop {
            task = self.outline()?.parent(task)?;
            if let Some(row) = self.timeline().row_of_task(task.0) {
                return Some(row);
            }
        }
    }
}
