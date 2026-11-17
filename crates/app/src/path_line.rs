//! The longest-path line (gantt#703): where the path starts and ends, how many activities it holds
//! and how many calendar days it spans, for the line at the chart's bottom left.

use crate::json::quote;
use crate::outline::Outline;
use crate::{RowFilter, Session};
use engine::{DAY, TaskId};

/// The longest path's activities (leaf tasks): the first (earliest start, lowest id of equals), the
/// last (latest finish), the count and the calendar days from the first start to the project finish.
#[derive(Clone, Copy)]
pub(crate) struct PathLine(TaskId, TaskId, usize, i64);

impl Session {
    /// The line for the current solution (read on a row rebuild): `None` unless Longest path is on
    /// and the path holds an activity.
    pub(crate) fn path_line(&self, outline: &Outline) -> Option<PathLine> {
        if self.row_filter != RowFilter::LongestPath {
            return None;
        }
        let dates = &self.solution.dates;
        let mut leaves = self.schedule.longest_path(&self.solution);
        leaves.retain(|&task| !outline.is_summary(task));
        let first = *leaves.iter().min_by_key(|t| dates[t.0 as usize].es)?;
        let last = *leaves.iter().max_by_key(|t| dates[t.0 as usize].ef)?;
        let day = |t: i64| t.div_euclid(DAY);
        let days = day(self.solution.project_finish) - day(dates[first.0 as usize].es);
        Some(PathLine(first, last, leaves.len(), days))
    }

    /// The status's `path`: `{"first","last","count","days"}`, or `null`.
    pub(crate) fn path_line_json(&self) -> String {
        self.path_line
            .map_or("null".to_string(), |PathLine(first, last, count, days)| {
                let (first, last) = (self.task_label(first.0), self.task_label(last.0));
                format!(
                    r#"{{"first":{},"last":{},"count":{count},"days":{days}}}"#,
                    quote(first),
                    quote(last)
                )
            })
    }
}
