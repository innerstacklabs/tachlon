//! The hover card's Calendar row (gs#703): the days a week and hours a day of the calendar a
//! task works to, so a float or a duration has a meaning; and the text of its activity code rows.

use crate::Session;
use crate::json::quote;
use crate::texts::{
    CARD_FLOAT_MANY, CARD_FLOAT_ONE, CARD_MISSED, CARD_MISSED_UNDER_DAY, CARD_MORE,
    CARD_NO_WORKING_DAYS, CARD_WEEK_MANY, CARD_WEEK_ONE, Texts,
};
use engine::{Calendar, DAY, TaskId};

/// `5 days a week, 8 h a day` for a calendar, in `texts`' words. The days are the weekdays with
/// any working time; the hours a day are the week's working time spread over them, to a tenth
/// when not whole.
fn calendar_text(texts: &Texts, calendar: &Calendar) -> String {
    let days = calendar.week().iter().filter(|day| !day.is_empty()).count();
    if days == 0 {
        return texts.say(&CARD_NO_WORKING_DAYS, &[]);
    }
    #[expect(
        clippy::cast_possible_wrap,
        reason = "days counts the weekdays that have a working period, at most 7"
    )]
    let tenths = (calendar.weekly_minutes() * 10 + 30 * days as i64) / (60 * days as i64);
    let hours = if tenths % 10 == 0 {
        (tenths / 10).to_string()
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    };
    let count = days.to_string();
    let holes = [("days", count.as_str()), ("hours", hours.as_str())];
    texts.say_by(days == 1, &CARD_WEEK_ONE, &CARD_WEEK_MANY, &holes)
}

impl Session {
    /// The calendar of task `i` as `5 days a week, 8 h a day`, or an empty string if the task is
    /// absent or deleted.
    pub fn task_calendar_text(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        calendar_text(&self.texts, self.schedule().calendar_of(TaskId(i)))
    }
}

impl Session {
    /// The total float of task `i` in calendar days, as `7 calendar days`: the whole days from its
    /// early finish to its late finish. An empty string if that is under a day, or the task is
    /// absent or deleted, so a task with no float has no such text.
    pub fn task_float_calendar_text(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        let Some(dates) = self.solution().dates.get(i as usize) else {
            return String::new();
        };
        match (dates.lf - dates.ef).div_euclid(DAY) {
            days if days <= 0 => String::new(),
            days => self.texts.say_by(
                days == 1,
                &CARD_FLOAT_ONE,
                &CARD_FLOAT_MANY,
                &[("days", &days.to_string())],
            ),
        }
    }
}

impl Session {
    /// The start-no-earlier-than of task `i` as `YYYY-MM-DD HH:MM`, the way the card writes dates,
    /// or an empty string if the task has no real constraint ([`Session::start_constraint`]: not
    /// one that only holds its import floor), is absent or is deleted.
    pub fn task_constraint_text(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        self.start_constraint(TaskId(i))
            .map_or_default(|t| self.date_text(t))
    }
}

impl Session {
    /// The predecessors whose links set task `i`'s early start, one outline path a line, the first
    /// three and then `+ N more`; an empty string if none does (a task that starts at the project
    /// start or at its own constraint) or the task is absent or deleted.
    pub fn task_driven_by(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        let drivers = self
            .schedule
            .driving_predecessors(&self.solution, TaskId(i));
        let mut lines: Vec<String> = drivers
            .iter()
            .take(3)
            .map(|pred| self.outline_path(pred.0))
            .collect();
        if drivers.len() > 3 {
            let more = (drivers.len() - 3).to_string();
            lines.push(self.texts.say(&CARD_MORE, &[("count", &more)]));
        }
        lines.join("\n")
    }
}

impl Session {
    /// The MS Project deadline of task `i` as `YYYY-MM-DD HH:MM`, followed by `(missed by 2d)`, the
    /// whole days its early finish falls after it (`(missed by <1d)` under a day). An empty string
    /// when the file gave the task no deadline, or the task is absent or deleted.
    pub fn task_deadline_text(&self, i: u32) -> String {
        let deadline = self.deadline(TaskId(i)).filter(|_| self.is_live(TaskId(i)));
        let (Some(deadline), Some(dates)) = (deadline, self.solution().dates.get(i as usize))
        else {
            return String::new();
        };
        let date = self.date_text(deadline);
        match (dates.ef - deadline).div_euclid(DAY) {
            _ if dates.ef <= deadline => date,
            0 => self.texts.say(&CARD_MISSED_UNDER_DAY, &[("date", &date)]),
            days => self.texts.say(
                &CARD_MISSED,
                &[("date", &date), ("days", &days.to_string())],
            ),
        }
    }
}

impl Session {
    /// The activity codes of task `i` as a JSON array of `[type, value]` in file order, one hover
    /// card row each. `[]` for a task with no activity code, or one that is absent or deleted.
    pub fn task_activity_codes(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return "[]".to_string();
        }
        let codes = (self.meta().get(i as usize)).map_or(&[][..], |meta| &meta.activity_codes);
        let pairs: Vec<String> = codes
            .iter()
            .map(|code| format!("[{},{}]", quote(&code.code_type), quote(&code.value)))
            .collect();
        format!("[{}]", pairs.join(","))
    }
}
