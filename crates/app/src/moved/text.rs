//! The words of a move report, in the chart's [`Texts`].

use crate::texts::{
    MOVED_CRITICAL, MOVED_DATA_DATE_CLEARED, MOVED_DATA_DATE_MANY, MOVED_DATA_DATE_MOVED,
    MOVED_DATA_DATE_ONE, MOVED_DATA_DATE_SET, MOVED_FINISH_MANY, MOVED_FINISH_ONE,
    MOVED_TASKS_MANY, MOVED_TASKS_ONE, Texts,
};
use std::fmt::Write as _;

/// What a data date change did to the date, for the head of the note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DataDate {
    /// The date went from one day to another: calendar days later (negative: earlier).
    Shifted(i64),
    /// There was a date and now there is none.
    Cleared,
    /// There was none and now there is one.
    Set,
}

fn data_date_clause(texts: &Texts, cause: DataDate) -> String {
    match cause {
        DataDate::Shifted(0) => texts.say(&MOVED_DATA_DATE_MOVED, &[]),
        DataDate::Shifted(days) => texts.say_by(
            days.unsigned_abs() == 1,
            &MOVED_DATA_DATE_ONE,
            &MOVED_DATA_DATE_MANY,
            &[("days", &format!("{days:+}"))],
        ),
        DataDate::Cleared => texts.say(&MOVED_DATA_DATE_CLEARED, &[]),
        DataDate::Set => texts.say(&MOVED_DATA_DATE_SET, &[]),
    }
}

/// A data date change opens the note: `"data date +7 days · "`. Then
/// `"{n} task(s) moved"`, then `" · finish +{d} day(s)"` or `" · finish -{d} day(s)"` when the
/// project finish moved by `finish_days` (a calendar-day difference, nonzero). A finish that
/// stayed on its day leaves the clause out. `" · critical {joined} joined, {left} left"` follows
/// when either count of tasks that joined or left the critical path is nonzero, in the words of
/// the baseline status line. The sentences are `texts`'; the ` · ` between them is not.
pub(super) fn summary(
    texts: &Texts,
    cause: Option<DataDate>,
    others: u32,
    finish_days: i64,
    joined: u32,
    left: u32,
) -> String {
    let mut text = cause.map_or_else(String::new, |c| {
        format!("{} · ", data_date_clause(texts, c))
    });
    let count = others.to_string();
    text += &texts.say_by(
        others == 1,
        &MOVED_TASKS_ONE,
        &MOVED_TASKS_MANY,
        &[("count", &count)],
    );
    if finish_days != 0 {
        let days = format!("{finish_days:+}");
        let finish = texts.say_by(
            finish_days.unsigned_abs() == 1,
            &MOVED_FINISH_ONE,
            &MOVED_FINISH_MANY,
            &[("days", &days)],
        );
        let _ = write!(text, " · {finish}");
    }
    if joined != 0 || left != 0 {
        let (joined, left) = (joined.to_string(), left.to_string());
        let holes = [("joined", joined.as_str()), ("left", left.as_str())];
        let _ = write!(text, " · {}", texts.say(&MOVED_CRITICAL, &holes));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::DataDate;
    use crate::texts::Texts;

    fn summary(
        cause: Option<DataDate>,
        others: u32,
        finish: i64,
        joined: u32,
        left: u32,
    ) -> String {
        super::summary(&Texts::default(), cause, others, finish, joined, left)
    }

    #[test]
    fn the_text_counts_tasks_and_names_the_finish_move() {
        assert_eq!(summary(None, 8, 7, 0, 0), "8 tasks moved · finish +7 days");
        assert_eq!(summary(None, 1, 0, 0, 0), "1 task moved");
        assert_eq!(summary(None, 2, 1, 0, 0), "2 tasks moved · finish +1 day");
        assert_eq!(summary(None, 8, -3, 0, 0), "8 tasks moved · finish -3 days");
        assert_eq!(summary(None, 3, -1, 0, 0), "3 tasks moved · finish -1 day");
    }

    #[test]
    fn a_data_date_change_opens_the_note() {
        let head = |c| summary(Some(c), 2, 3, 0, 0);
        assert_eq!(
            head(DataDate::Shifted(7)),
            "data date +7 days · 2 tasks moved · finish +3 days"
        );
        assert!(head(DataDate::Shifted(-1)).starts_with("data date -1 day · "));
        assert!(head(DataDate::Shifted(0)).starts_with("data date moved · "));
        assert!(head(DataDate::Cleared).starts_with("data date cleared · "));
        assert!(head(DataDate::Set).starts_with("data date set · "));
    }

    #[test]
    fn the_text_names_the_critical_path_change_after_the_finish() {
        assert_eq!(
            summary(None, 8, 7, 2, 1),
            "8 tasks moved · finish +7 days · critical 2 joined, 1 left"
        );
        assert_eq!(
            summary(None, 1, 0, 0, 3),
            "1 task moved · critical 0 joined, 3 left"
        );
        assert_eq!(
            summary(None, 2, -1, 1, 0),
            "2 tasks moved · finish -1 day · critical 1 joined, 0 left"
        );
        assert_eq!(
            summary(None, 4, 0, 0, 0),
            "4 tasks moved",
            "no change, no clause"
        );
    }
}
