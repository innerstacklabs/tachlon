//! The critical tasks and the longest path, listed for a program (gantt#1250). The session is read, never changed.

use crate::{Session, explain::refuse, health::HEALTH_MAX};
use engine::TaskId;

impl Session {
    /// The critical tasks (zero total float, not finished) and the longest path's tasks, each only the leaves the Critical and
    /// Longest path row filters keep (no summary, no deleted task), in task order, answered as
    /// `{"ok":true,"critical":list,"longest":list}`, a `list` being `{"count":n,"tasks":[{"task","id","name"}]}` with `count` the
    /// true total and `tasks` its first 50; or a `"busy"` refusal.
    #[must_use]
    pub fn critical_path(&self) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        let (schedule, solution) = (self.schedule(), self.solution());
        let leaf = |n: u32| {
            !schedule.tasks()[n as usize].deleted
                && !self
                    .outline()
                    .is_some_and(|outline| outline.is_summary(TaskId(n)))
        };
        let critical: Vec<u32> = (0..)
            .zip(&solution.dates)
            .filter(|&(n, dates)| leaf(n) && dates.is_critical())
            .map(|(n, _)| n)
            .collect();
        let longest: Vec<u32> = schedule
            .longest_path(solution)
            .into_iter()
            .map(|task| task.0)
            .filter(|&n| leaf(n))
            .collect();
        format!(
            r#"{{"ok":true,"critical":{},"longest":{}}}"#,
            self.task_list(&critical),
            self.task_list(&longest)
        )
    }

    /// `{"count":n,"tasks":[{"task","id","name"}]}` for the slots `members`: their number and the first 50.
    fn task_list(&self, members: &[u32]) -> String {
        let tasks: Vec<String> = members
            .iter()
            .take(HEALTH_MAX)
            .map(|&n| self.health_task(n))
            .collect();
        format!(
            r#"{{"count":{},"tasks":[{}]}}"#,
            members.len(),
            tasks.join(",")
        )
    }
}
