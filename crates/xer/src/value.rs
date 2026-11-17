/// The outcome of parsing one decoded XER cell.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Parsed<T> {
    /// The cell contains no value after trimming ASCII whitespace.
    Empty,
    /// The cell contains a valid value.
    Value(T),
    /// The cell is non-empty but does not contain a valid value.
    Bad,
}

/// Parses an unsigned 32-bit integer containing only ASCII digits.
pub(crate) fn parse_u32(text: &str) -> Parsed<u32> {
    let text = trim(text);
    if text.is_empty() {
        return Parsed::Empty;
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Parsed::Bad;
    }

    match text.parse() {
        Ok(value) => Parsed::Value(value),
        Err(_) => Parsed::Bad,
    }
}

/// Parses a finite number of hours.
///
/// Only `.` is accepted as a decimal point. Public English-locale samples use it (even beside a `,`
/// currency symbol); the convention is unverified for
/// exports produced with non-English locale settings.
pub(crate) fn parse_hours(text: &str) -> Parsed<f64> {
    let text = trim(text);
    if text.is_empty() {
        return Parsed::Empty;
    }

    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => Parsed::Value(value),
        Ok(_) | Err(_) => Parsed::Bad,
    }
}

/// Parses an XER date or date-time as wall-clock minutes since Unix epoch.
///
/// Accepted forms are `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, and
/// `YYYY-MM-DD HH:MM:SS`. Seconds are validated and then discarded.
pub(crate) fn parse_minutes(text: &str) -> Parsed<i64> {
    let text = trim(text);
    if text.is_empty() {
        return Parsed::Empty;
    }

    let Some((year, month, day, hour, minute)) = date_time_parts(text) else {
        return Parsed::Bad;
    };
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
    {
        return Parsed::Bad;
    }

    let days = days_from_civil(year, month, day);
    let minutes = days * 1_440 + i64::from(hour) * 60 + i64::from(minute);
    Parsed::Value(minutes)
}

/// Parses an XER `Y` or `N` flag.
pub(crate) fn parse_flag(text: &str) -> Parsed<bool> {
    match trim(text) {
        "" => Parsed::Empty,
        "Y" => Parsed::Value(true),
        "N" => Parsed::Value(false),
        _ => Parsed::Bad,
    }
}

fn trim(text: &str) -> &str {
    text.trim_matches(|character: char| character.is_ascii_whitespace())
}

fn date_time_parts(text: &str) -> Option<(u32, u32, u32, u32, u32)> {
    if !matches!(text.len(), 10 | 16 | 19) {
        return None;
    }
    let bytes = text.as_bytes();
    if bytes.get(4).copied() != Some(b'-')
        || bytes.get(7).copied() != Some(b'-')
        || (text.len() > 10
            && (bytes.get(10).copied() != Some(b' ') || bytes.get(13).copied() != Some(b':')))
        || (text.len() == 19 && bytes.get(16).copied() != Some(b':'))
    {
        return None;
    }

    let year = fixed_digits(text.get(..4)?)?;
    let month = fixed_digits(text.get(5..7)?)?;
    let day = fixed_digits(text.get(8..10)?)?;
    let (hour, minute) = if text.len() == 10 {
        (0, 0)
    } else {
        let hour = fixed_digits(text.get(11..13)?)?;
        let minute = fixed_digits(text.get(14..16)?)?;
        (hour, minute)
    };
    if text.len() == 19 {
        let second = fixed_digits(text.get(17..19)?)?;
        if second > 59 {
            return None;
        }
    }

    Some((year, month, day, hour, minute))
}

fn fixed_digits(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

// Howard Hinnant's civil-calendar algorithm, shifted to 1970-01-01.
fn days_from_civil(year: u32, month: u32, day: u32) -> i64 {
    let mut year = i64::from(year);
    let month = i64::from(month);
    let day = i64::from(day);
    if month <= 2 {
        year -= 1;
    }
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::{Parsed, parse_flag, parse_hours, parse_minutes, parse_u32};

    #[test]
    fn unsigned_integers_require_ascii_digits_and_fit() {
        assert_eq!(parse_u32("42"), Parsed::Value(42));
        assert_eq!(parse_u32("+5"), Parsed::Bad);
        assert_eq!(parse_u32("4294967296"), Parsed::Bad);
        assert_eq!(parse_u32("１２"), Parsed::Bad);
    }

    #[test]
    fn hours_are_finite_and_use_a_dot_decimal_point() {
        assert_eq!(parse_hours("1.5"), Parsed::Value(1.5));
        assert_eq!(parse_hours("1,5"), Parsed::Bad);
        assert_eq!(parse_hours("NaN"), Parsed::Bad);
        assert_eq!(parse_hours("inf"), Parsed::Bad);
        assert_eq!(parse_hours("-inf"), Parsed::Bad);
    }

    #[test]
    fn dates_and_times_become_epoch_minutes() {
        assert_eq!(parse_minutes("1970-01-01 00:00"), Parsed::Value(0));
        assert_eq!(parse_minutes("2000-03-01 00:00"), Parsed::Value(15_864_480));
        assert_eq!(
            parse_minutes("2000-02-29 12:34:56"),
            Parsed::Value(15_863_794)
        );
        assert_eq!(parse_minutes("1970-01-01"), Parsed::Value(0));
    }

    #[test]
    fn dates_and_times_reject_invalid_calendar_values() {
        assert_eq!(parse_minutes("2023-02-29 00:00"), Parsed::Bad);
        assert_eq!(parse_minutes("2024-01-01 24:00"), Parsed::Bad);
        assert_eq!(parse_minutes("0000-01-01"), Parsed::Bad);
        assert_eq!(parse_minutes("2024-01-01 00:00:60"), Parsed::Bad);
    }

    #[test]
    fn flags_accept_only_y_and_n() {
        assert_eq!(parse_flag("Y"), Parsed::Value(true));
        assert_eq!(parse_flag("N"), Parsed::Value(false));
        assert_eq!(parse_flag("y"), Parsed::Bad);
        assert_eq!(parse_flag("yes"), Parsed::Bad);
    }

    #[test]
    fn empty_and_ascii_whitespace_are_handled_consistently() {
        assert_eq!(parse_u32("\t 5\r\n"), Parsed::Value(5));
        assert_eq!(parse_hours("\t 2.5 "), Parsed::Value(2.5));
        assert_eq!(parse_minutes("\r 1970-01-01 00:01\n"), Parsed::Value(1));
        assert_eq!(parse_flag("\tY "), Parsed::Value(true));

        assert_eq!(parse_u32(" \t\r\n"), Parsed::Empty);
        assert_eq!(parse_hours(""), Parsed::Empty);
        assert_eq!(parse_minutes(" "), Parsed::Empty);
        assert_eq!(parse_flag("\t"), Parsed::Empty);
    }
}
