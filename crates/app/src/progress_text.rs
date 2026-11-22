//! The task table's progress columns (gs#703): Actual start, Actual finish, Remaining and
//! % complete, read from what the engine holds. Percent is derived from the remaining and the
//! duration (decision 0008, pick 1). Work that has not started reads as empty text: the engine
//! ignores a remaining until the task has an actual start.

use crate::{Session, edit};
use engine::{Minutes, Task, TaskId};

impl Session {
    /// The task `i` when it is live, else `None`.
    pub(crate) fn live_task(&self, i: u32) -> Option<&Task> {
        if !self.is_live(TaskId(i)) {
            return None;
        }
        self.schedule().tasks().get(i as usize)
    }

    /// Whether the task has begun: an actual start, or an actual finish (a finish alone is a task
    /// that is done).
    fn has_begun(task: &Task) -> bool {
        task.actual_start.is_some() || task.actual_finish.is_some()
    }

    /// The working minutes still to do: nothing once finished, else the stored remaining, else
    /// the whole duration. `None` for work not started.
    pub(crate) fn remaining_minutes(task: &Task) -> Option<Minutes> {
        if !Self::has_begun(task) {
            return None;
        }
        if task.actual_finish.is_some() {
            return Some(0);
        }
        Some(
            task.remaining
                .unwrap_or(task.duration)
                .clamp(0, task.duration),
        )
    }

    /// The actual start of task `i` as the Start column writes dates, or an empty string if it
    /// has none or is absent or deleted.
    pub fn task_actual_start(&self, i: u32) -> String {
        self.live_task(i)
            .and_then(|task| task.actual_start)
            .map_or_else(String::new, |t| self.date_text(t))
    }

    /// The actual finish of task `i` as the Finish column writes dates, or an empty string if it
    /// has none or is absent or deleted.
    pub fn task_actual_finish(&self, i: u32) -> String {
        self.live_task(i)
            .and_then(|task| task.actual_finish)
            .map_or_else(String::new, |t| self.date_text(t))
    }

    /// The remaining duration of task `i` in the Dur column's format, or an empty string for work
    /// not started or a task that is absent or deleted.
    pub fn task_remaining(&self, i: u32) -> String {
        let Some(task) = self.live_task(i) else {
            return String::new();
        };
        let Some(remaining) = Self::remaining_minutes(task) else {
            return String::new();
        };
        let day = edit::day_minutes(self.schedule().calendar_of(TaskId(i)));
        edit::format_duration(remaining, day)
    }

    /// The percent complete of task `i` as a whole number with `%`, or an empty string for work
    /// not started or a task that is absent or deleted. A milestone reads `100%` once finished
    /// and empty before. A summary reads the share of the work under it that is done, weighted by
    /// duration (`progress_rollup`), and empty while no leaf under it has begun.
    /// A percent a person typed reads as typed while the remaining work it set is unchanged
    /// (the remaining rounds to whole hours, so deriving it back would read differently).
    pub fn task_percent(&self, i: u32) -> String {
        let Some(task) = self.live_task(i) else {
            return String::new();
        };
        if self.outline().is_some_and(|o| o.is_summary(TaskId(i))) {
            return self.summary_percent(TaskId(i));
        }
        let Some(remaining) = Self::remaining_minutes(task) else {
            return String::new();
        };
        if task.duration <= 0 {
            return if task.actual_finish.is_some() {
                "100%".to_owned()
            } else {
                String::new()
            };
        }
        if let Some(typed) = self.typed_percent_shown(i) {
            return format!("{typed}%");
        }
        let done = i128::from(task.duration - remaining) * 100;
        let percent = (done * 2 + i128::from(task.duration)) / (2 * i128::from(task.duration));
        format!("{percent}%")
    }

    /// The physical percent complete of task `i` beside the derived one (`task_percent`): `50%`,
    /// `12.5%` or `12.35%`, or an empty string for a task with none, a summary and a task that is
    /// absent or deleted.
    pub fn task_physical(&self, i: u32) -> String {
        let physical = self
            .live_task(i)
            .and_then(|_| self.meta().get(i as usize))
            .and_then(|meta| meta.physical);
        let Some(hundredths) = physical else {
            return String::new();
        };
        if self.outline().is_some_and(|o| o.is_summary(TaskId(i))) {
            return String::new();
        }
        let text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
        format!("{}%", text.trim_end_matches('0').trim_end_matches('.'))
    }

    /// Whether the open schedule carries progress: a live task with an actual start, an actual
    /// finish. A remaining alone (an XER states one on tasks not yet started, whose cells stay
    /// empty) does not count, nor does a percent complete alone (the timeline's display value,
    /// which the engine does not hold). The table shows the progress columns only then. It scans every task, so
    /// the page asks once per opened file, not per frame.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    pub fn has_progress(&self) -> bool {
        self.schedule().tasks().iter().enumerate().any(|(i, task)| {
            self.is_live(TaskId(i as u32))
                && (task.actual_start.is_some() || task.actual_finish.is_some())
        })
    }
}
