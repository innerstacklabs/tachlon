//! The variance cells of the task table (gs#703): how far a task's dates and float are from the
//! baseline, as the text the page shows.

use crate::Session;
use crate::edit::{day_minutes, format_float_days};
use engine::{Instant, Minutes, TaskId};

impl Session {
    /// A table row's Start, Finish and Float variance in working days of the task's own calendar
    /// (`+5d`, `-2d`, `0`); positive is later or more float. `None` with no baseline or entry; the
    /// float is `None` for a baseline a file saved, which has none, and a date is
    /// `None` when the baseline's is before the origin.
    pub(crate) fn variance_cells(&self, task: TaskId) -> Option<[Option<String>; 3]> {
        let was = self.baseline.as_ref()?.task(task)?;
        let now = &self.solution.dates[task.0 as usize];
        let calendar = self.schedule.calendar_of(task);
        let day = day_minutes(calendar);
        let text = |m: Minutes| match m {
            0 => "0".to_string(),
            1.. => format!("+{}", format_float_days(m, day)),
            _ => format_float_days(m, day),
        };
        // `Calendar::count` needs an instant on or after the origin; a baseline a file saved can hold an
        // earlier date, and that task's date has no working-day change to show.
        let moved = |now: Instant, was: Instant| {
            (was >= 0).then(|| text(calendar.count(now) - calendar.count(was)))
        };
        Some([
            moved(now.es, was.es),
            moved(now.ef, was.ef),
            was.float.map(|float| text(now.float - float)),
        ])
    }
}

impl Session {
    /// The hover card's baseline lines for a leaf: the baseline's start and finish as date text and the
    /// start and finish variance as `variance_cells` writes them. All `None` for a summary, with no
    /// baseline or entry, and a date is `None` when the baseline's is before the origin.
    pub(crate) fn baseline_card(&self, task: TaskId) -> BaselineCard {
        let (Some([start, finish, _]), Some(was)) = (
            self.variance_cells(task)
                .filter(|_| self.summary_span(task.0).is_none()),
            self.baseline.as_ref().and_then(|b| b.task(task)),
        ) else {
            return (None, None, None);
        };
        let date = |at: Instant| (at >= 0).then(|| self.date_text(at));
        (date(was.es), date(was.ef), Some([start, finish]))
    }
}

/// The hover card's baseline start, finish and the two variances.
pub(crate) type BaselineCard = (Option<String>, Option<String>, Option<[Option<String>; 2]>);
