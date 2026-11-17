//! The task rows beside a task on screen while grouped (gantt#2029): a delete selects the next
//! row the user sees, not the next leaf of the outline, which can sit in another group.

use crate::Session;

impl Session {
    /// The tasks on the task rows below and above `task`'s row on screen, over headers; `None`
    /// for a side with no task row, and both `None` while ungrouped or when `task` has no row.
    pub(crate) fn screen_neighbors(&self, task: u32) -> [Option<u32>; 2] {
        let tl = self.timeline();
        let Some(row) = tl.row_of_task(task).filter(|_| self.group.is_some()) else {
            return [None; 2];
        };
        [1, -1].map(|delta| {
            let to = tl.step_task_row(Some(row), delta).filter(|&to| to != row)?;
            tl.task_at_row(to)
        })
    }
}
