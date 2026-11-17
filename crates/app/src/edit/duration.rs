//! Typed durations and the day-based text the table shows: parse, format, and the float column.

use crate::texts::{
    DURATION_EMPTY, DURATION_INVALID, DURATION_NEGATIVE, DURATION_TOO_LONG, Texts, Worded,
    english_display,
};
use engine::{Calendar, Minutes, Schedule, TaskId};

/// Why a typed duration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationError {
    Empty,
    Negative,
    /// Not `3d`, `4h`, `30m` or a combination such as `1d 4h`.
    Invalid,
    TooLong,
}

impl Worded for DurationError {
    fn text(&self, texts: &Texts) -> String {
        texts.say(
            match self {
                Self::Empty => &DURATION_EMPTY,
                Self::Negative => &DURATION_NEGATIVE,
                Self::Invalid => &DURATION_INVALID,
                Self::TooLong => &DURATION_TOO_LONG,
            },
            &[],
        )
    }
}
english_display!(DurationError);

impl std::error::Error for DurationError {}

/// Working minutes in one "day" of `calendar`: its weekly minutes over the
/// weekdays that have a working period (1,440 for a calendar with none).
#[must_use]
pub fn day_minutes(calendar: &Calendar) -> Minutes {
    #[expect(clippy::cast_possible_wrap, reason = "a count of weekdays, at most 7")]
    let working_days = calendar.week().iter().filter(|day| !day.is_empty()).count() as i64;
    if working_days == 0 {
        1440
    } else {
        (calendar.weekly_minutes() / working_days).max(1)
    }
}

/// Parse a typed duration into working minutes. A day is `day_minutes` of the
/// task's calendar. Accepts `3d`, `4h`, `30m`, any of them together (`1d 4h`,
/// `1d4h`; each unit once, any order), a lone number for that many days (`5` is `5d`) and a bare
/// `0` for a milestone.
///
/// # Errors
/// A [`DurationError`] for empty, negative or malformed text, and for a span too long to count.
pub fn parse_duration(text: &str, day_minutes: Minutes) -> Result<Minutes, DurationError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    if text.starts_with('-') {
        return Err(DurationError::Negative);
    }
    let day = day_minutes.max(1);
    let mut chars = text.chars().peekable();
    let mut total: Minutes = 0;
    let mut seen = [false; 3];
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.peek().is_none() {
            break;
        }
        let mut digits = String::new();
        while let Some(c) = chars.next_if(char::is_ascii_digit) {
            digits.push(c);
        }
        if digits.is_empty() {
            return Err(DurationError::Invalid);
        }
        let amount: Minutes = digits.parse().map_err(|_| DurationError::TooLong)?;
        let (slot, unit) = match chars.next() {
            Some('d') => (0, day),
            Some('h') => (1, 60),
            Some('m') => (2, 1),
            // A number with no unit: a lone one is days (a lone zero is a milestone).
            None if !seen.iter().any(|&was_seen| was_seen) => {
                return amount.checked_mul(day).ok_or(DurationError::TooLong);
            }
            _ => return Err(DurationError::Invalid),
        };
        if std::mem::replace(&mut seen[slot], true) {
            return Err(DurationError::Invalid);
        }
        total = amount
            .checked_mul(unit)
            .and_then(|minutes| total.checked_add(minutes))
            .ok_or(DurationError::TooLong)?;
    }
    Ok(total)
}

/// A Dur cell's text: a duration, or `+` or `-` and a duration to add to or take from the task's.
///
/// # Errors
/// As [`parse_duration`], and `Negative` for a result below zero.
pub fn parse_duration_cell(
    text: &str,
    schedule: &Schedule,
    task: TaskId,
) -> Result<Minutes, DurationError> {
    let day = day_minutes(schedule.calendar_of(task));
    let text = text.trim();
    let Some(change) = text.strip_prefix(['+', '-']) else {
        return parse_duration(text, day);
    };
    let current = schedule.tasks()[task.0 as usize].duration;
    let change = parse_duration(change, day)? * if text.starts_with('+') { 1 } else { -1 };
    match current.checked_add(change) {
        Some(minutes) if minutes >= 0 => Ok(minutes),
        Some(_) => Err(DurationError::Negative),
        None => Err(DurationError::TooLong),
    }
}

/// A duration as the table shows it and [`parse_duration`] reads it back:
/// `0` for a milestone, else the nonzero parts of `Nd Nh Nm`.
#[must_use]
pub fn format_duration(minutes: Minutes, day_minutes: Minutes) -> String {
    if minutes <= 0 {
        return "0".to_string();
    }
    let day = day_minutes.max(1);
    let parts = [
        (minutes / day, 'd'),
        (minutes % day / 60, 'h'),
        (minutes % day % 60, 'm'),
    ];
    parts
        .iter()
        .filter(|(amount, _)| *amount != 0)
        .map(|(amount, unit)| format!("{amount}{unit}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Total float as the table shows it: working days of the task's calendar,
/// with the suffix `d`. A whole number of days shows as `3d`. Any other value
/// rounds to the nearest tenth (`2.5d`) below 100 days and to a whole day from
/// 100 days up (`16698d`, so the column stays narrow), and a nonzero float under
/// 0.05 of a day shows as `<0.1d` (or `>-0.1d`), so only an exact zero reads `0d`.
#[must_use]
pub fn format_float_days(minutes: Minutes, day_minutes: Minutes) -> String {
    if minutes == 0 {
        return "0d".to_string();
    }
    let day = day_minutes.max(1);
    let sign = if minutes < 0 { "-" } else { "" };
    let magnitude = minutes.unsigned_abs();
    let day = day.unsigned_abs();
    if magnitude.is_multiple_of(day) {
        return format!("{sign}{}d", magnitude / day);
    }
    // Tenths of a day, rounded half up, in integers.
    let tenths = (magnitude * 10 + day / 2) / day;
    if tenths == 0 {
        return if minutes < 0 { ">-0.1d" } else { "<0.1d" }.to_string();
    }
    if tenths >= 1000 {
        return format!("{sign}{}d", (magnitude + day / 2) / day);
    }
    if tenths.is_multiple_of(10) {
        // Rounds to a whole number of days but is not one: keep the decimal.
        return format!("{sign}{}.0d", tenths / 10);
    }
    format!("{sign}{}.{}d", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Minutes = 480;

    #[test]
    fn float_shows_working_days_of_the_calendar() {
        assert_eq!(format_float_days(0, DAY), "0d");
        assert_eq!(format_float_days(3 * DAY, DAY), "3d");
        assert_eq!(format_float_days(12 * DAY, DAY), "12d");
        // The same minutes are a different number of days on a 24-hour calendar.
        assert_eq!(format_float_days(3 * DAY, 1440), "1d");
        assert_eq!(format_float_days(DAY, 1440), "0.3d");
        assert_eq!(format_float_days(2 * DAY + DAY / 2, DAY), "2.5d");
        assert_eq!(format_float_days(-2 * DAY, DAY), "-2d");
        assert_eq!(format_float_days(-(DAY + DAY / 2), DAY), "-1.5d");
    }

    #[test]
    fn float_never_reads_as_zero_unless_it_is() {
        assert_eq!(format_float_days(1, DAY), "<0.1d");
        assert_eq!(format_float_days(-1, DAY), ">-0.1d");
        assert_eq!(format_float_days(DAY - 1, DAY), "1.0d");
        assert_eq!(format_float_days(DAY / 10, DAY), "0.1d");
        assert_eq!(format_float_days(3, 0), "3d");
        // From 100 days up the decimal goes, so the column stays narrow.
        assert_eq!(format_float_days(99 * DAY + DAY * 9 / 10, DAY), "99.9d");
        assert_eq!(format_float_days(99 * DAY + DAY * 99 / 100, DAY), "100d");
        assert_eq!(format_float_days(16697 * DAY + DAY / 2, DAY), "16698d");
        assert_eq!(format_float_days(-(16697 * DAY + DAY / 2), DAY), "-16698d");
    }

    #[test]
    fn parses_days_hours_both_and_zero() {
        assert_eq!(parse_duration("3d", DAY), Ok(3 * DAY));
        assert_eq!(parse_duration("4h", DAY), Ok(240));
        assert_eq!(parse_duration("1d 4h", DAY), Ok(DAY + 240));
        assert_eq!(parse_duration("1d4h", DAY), Ok(DAY + 240));
        assert_eq!(parse_duration("4h 1d", DAY), Ok(DAY + 240));
        assert_eq!(parse_duration("  2d  ", DAY), Ok(2 * DAY));
        assert_eq!(parse_duration("30m", DAY), Ok(30));
        assert_eq!(parse_duration("0", DAY), Ok(0));
        assert_eq!(parse_duration("0d", DAY), Ok(0));
    }

    #[test]
    fn a_lone_number_is_days_and_a_lone_zero_a_milestone() {
        assert_eq!(parse_duration("5", DAY), Ok(5 * DAY));
        assert_eq!(parse_duration(" 3 ", DAY), Ok(3 * DAY));
        assert_eq!(parse_duration("2", 1440), Ok(2 * 1440));
        assert_eq!(parse_duration("0", DAY), Ok(0));
        assert_eq!(
            parse_duration("9223372036854775807", DAY),
            Err(DurationError::TooLong)
        );
    }

    #[test]
    fn refuses_negative_unparseable_and_empty() {
        assert_eq!(parse_duration("-1d", DAY), Err(DurationError::Negative));
        assert_eq!(parse_duration("abc", DAY), Err(DurationError::Invalid));
        assert_eq!(parse_duration("", DAY), Err(DurationError::Empty));
        assert_eq!(parse_duration("   ", DAY), Err(DurationError::Empty));
        // A unit needs a number, a unit is given once, a number after a unit needs its own unit.
        assert_eq!(parse_duration("d", DAY), Err(DurationError::Invalid));
        assert_eq!(parse_duration("5 3", DAY), Err(DurationError::Invalid));
        assert_eq!(parse_duration("1d 2d", DAY), Err(DurationError::Invalid));
        assert_eq!(parse_duration("1.5d", DAY), Err(DurationError::Invalid));
        assert_eq!(parse_duration("1d 0", DAY), Err(DurationError::Invalid));
        assert_eq!(
            parse_duration("99999999999999999999d", DAY),
            Err(DurationError::TooLong)
        );
    }

    #[test]
    fn format_reads_back_through_the_parser() {
        for minutes in [0, 1, 59, 60, 61, 479, 480, 481, 540, 1440, 5000] {
            let text = format_duration(minutes, DAY);
            assert_eq!(parse_duration(&text, DAY), Ok(minutes), "{text}");
        }
        assert_eq!(format_duration(0, DAY), "0");
        assert_eq!(format_duration(DAY + 240, DAY), "1d 4h");
        assert_eq!(format_duration(196, DAY), "3h 16m");
    }
}
