//! What the hover card shows for one task.

use std::fmt::Write as _;

/// The float text the hover card shows (`2 d 3 h`, `1 d 2 h 5 min`): `float` working minutes split by
/// a working day of `day_len` minutes, written from the magnitude with one minus sign in front of a
/// negative float (`-1 d 2 h`, not `-1 d -2 h`).
pub(super) fn float_text(float: i64, day_len: i64) -> String {
    let magnitude = float.saturating_abs();
    let sign = if float < 0 { "-" } else { "" };
    let (days, rest) = (magnitude / day_len, magnitude % day_len);
    let mut text = format!("{sign}{days} d {} h", rest / 60);
    if rest % 60 != 0 {
        let _ = write!(text, " {} min", rest % 60);
    }
    text
}

/// Everything the hover card shows for one task.
///
/// Float is measured in working minutes on the task's calendar. Its working
/// day length is the calendar's weekly minutes divided by the number of
/// weekdays with at least one period (or 1,440 minutes when there are none);
/// `float_days`, `float_hours`, and `float_rem_minutes` split the total using
/// that day length and 60-minute hours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDetails {
    pub task: u32,
    /// The task's own name (`Pour slab`).
    pub name: String,
    /// The full outline path, derived by [`crate::Session::outline_path`]
    /// (`Level 3 / Concrete / Pour slab`).
    pub outline_path: String,
    /// The activity ID as the ID column shows it; empty when the task has none.
    pub activity_id: String,
    /// `Not started`, `In progress` or `Complete`, from the task's actuals.
    pub status: String,
    /// The actual start as the Start column writes dates; empty until set.
    pub actual_start: String,
    /// The actual finish as the Finish column writes dates; empty until set.
    pub actual_finish: String,
    /// The percent complete as the table derives it (`50%`); empty for work not started.
    pub percent_complete: String,
    pub start: String,
    pub finish: String,
    pub late_start: String,
    pub late_finish: String,
    /// Total float, in working minutes of the task's calendar.
    pub float_minutes: i64,
    pub float_days: i64,
    pub float_hours: i64,
    pub float_rem_minutes: i64,
    pub float_text: String,
    pub critical: bool,
    pub predecessor_count: u32,
    /// The first three distinct predecessor outline paths, in dependency order.
    pub predecessors: Vec<String>,
    pub successor_count: u32,
    /// The first three distinct successor outline paths, in dependency order.
    pub successors: Vec<String>,
    /// The baseline's start and finish dates, and their variance in working days (`+5d`); `None`
    /// for a summary, with no baseline or entry (gs#703).
    pub baseline_start: Option<String>,
    pub baseline_finish: Option<String>,
    pub baseline_variance: Option<[Option<String>; 2]>,
    /// The names of the task's resources, in the file's order; empty for a task with none (gs#703).
    pub resources: Vec<String>,
}
