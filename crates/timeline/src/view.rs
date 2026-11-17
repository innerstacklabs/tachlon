//! The rows on screen (decision 0015): the order itself, or while the list is grouped a view
//! whose header rows have no task. The order stays a list of tasks either way.

use crate::{HIDDEN_ROW, Timeline};

/// The task-to-row table for `rows`, at least `len` long; a header row ([`HIDDEN_ROW`]) has no
/// task to index.
pub(crate) fn row_index(rows: &[u32], len: usize) -> Vec<u32> {
    let tasks = || rows.iter().copied().filter(|&task| task != HIDDEN_ROW);
    let max_task = tasks().max().map_or(0, |task| task as usize + 1);
    let mut row_of = vec![HIDDEN_ROW; len.max(max_task)];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row is an index into the rows, which list tasks by their u32 ids once each, so it fits u32"
    )]
    for (row, &task) in rows.iter().enumerate() {
        if task != HIDDEN_ROW {
            row_of[task as usize] = row as u32;
        }
    }
    row_of
}

impl Timeline {
    /// The rows on screen: `rows()[r]` is the task on row r, or [`HIDDEN_ROW`] on a group
    /// header. It is the order while no view is set.
    #[must_use]
    pub fn rows(&self) -> &[u32] {
        if self.view.is_empty() {
            &self.order
        } else {
            &self.view
        }
    }

    /// The task on screen row `row`; `None` on a group header or past the rows.
    #[must_use]
    pub fn task_at_row(&self, row: usize) -> Option<u32> {
        let task = *self.rows().get(row)?;
        (task != HIDDEN_ROW).then_some(task)
    }

    /// The task row `delta` task rows from `row` (up when negative), over headers, stopping at the
    /// first or last. With no `row`, the first task row; `None` when no row holds a task.
    #[must_use]
    pub fn step_task_row(&self, row: Option<usize>, delta: i32) -> Option<usize> {
        let rows = self.rows();
        let task = |&r: &usize| rows[r] != HIDDEN_ROW;
        let Some(row) = row else {
            return (0..rows.len()).find(task);
        };
        let n = delta.unsigned_abs() as usize;
        let moved = if delta >= 0 {
            (row + 1..rows.len()).filter(task).take(n).last()
        } else {
            (0..row).rev().filter(task).take(n).last()
        };
        Some(moved.unwrap_or(row))
    }

    /// Show `view` on screen in place of the order: the order's tasks with [`HIDDEN_ROW`] for
    /// each group header. The task-to-row table follows it; an empty view shows the order.
    /// [`Timeline::set_order`] drops the view.
    pub fn set_view(&mut self, view: Vec<u32>) {
        self.view = view;
        self.row_of = row_index(self.rows(), self.row_of.len());
        self.viewport.rows = self.rows().len();
        self.viewport.scroll_y = self
            .viewport
            .scroll_y
            .clamp(0.0, self.viewport.max_scroll());
    }
}
