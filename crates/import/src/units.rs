pub(crate) fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let adjusted_year = i64::from(year) - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The `(year, month, day)` of a day count since 1970-01-01, the inverse of `days_from_civil`.
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era.saturating_mul(400) + i64::from(month <= 2);
    (year, month, day)
}

/// Format minutes since 1970-01-01 00:00 as `YYYY-MM-DD HH:MM`.
pub(crate) fn format_minutes(minutes: i64) -> String {
    let (year, month, day) = civil_from_days(minutes.div_euclid(1_440));
    let minute_of_day = minutes.rem_euclid(1_440);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minute_of_day / 60,
        minute_of_day % 60
    )
}

#[expect(
    clippy::cast_sign_loss,
    reason = "rem_euclid(7) is 0..=6, so adding 3 and taking the remainder by 7 gives 0..=6: not negative, fits u8"
)]
pub(crate) fn weekday_of(days_since_1970_01_01: i64) -> u8 {
    ((days_since_1970_01_01.rem_euclid(7) + 3) % 7) as u8
}

fn decimal(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0u32, |value, byte| {
        let digit = byte.checked_sub(b'0').filter(|digit| *digit <= 9)?;
        value.checked_mul(10)?.checked_add(u32::from(digit))
    })
}

fn leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn valid_date(year: i32, month: u32, day: u32) -> bool {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn valid_datetime_suffix(suffix: &[u8]) -> bool {
    let timezone = if suffix.is_empty() {
        return true;
    } else if suffix.first() == Some(&b'.') {
        let Some(after_dot) = suffix.get(1..) else {
            return false;
        };
        let fraction_end = after_dot
            .iter()
            .position(|byte| !byte.is_ascii_digit())
            .map_or(suffix.len(), |index| index + 1);
        if fraction_end == 1 {
            return false;
        }
        let Some(timezone) = suffix.get(fraction_end..) else {
            return false;
        };
        timezone
    } else {
        suffix
    };

    if timezone.is_empty() || timezone == b"Z" {
        return true;
    }
    timezone.len() == 6
        && matches!(timezone.first(), Some(b'+' | b'-'))
        && timezone.get(3) == Some(&b':')
        && decimal(timezone.get(1..3).unwrap_or_default()).is_some_and(|hour| hour <= 23)
        && decimal(timezone.get(4..6).unwrap_or_default()).is_some_and(|minute| minute <= 59)
}

pub(crate) fn parse_datetime(value: &str) -> Option<(i64, u32)> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }

    let year = i32::try_from(decimal(bytes.get(0..4)?)?).ok()?;
    let month = decimal(bytes.get(5..7)?)?;
    let day = decimal(bytes.get(8..10)?)?;
    if !valid_date(year, month, day) {
        return None;
    }
    let days = days_from_civil(year, month, day);

    if bytes.len() == 10 {
        return Some((days, 0));
    }
    if bytes.len() < 19
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let hour = decimal(bytes.get(11..13)?)?;
    let minute = decimal(bytes.get(14..16)?)?;
    let second = decimal(bytes.get(17..19)?)?;
    if hour > 23 || minute > 59 || second > 59 || !valid_datetime_suffix(bytes.get(19..)?) {
        return None;
    }
    let minute_of_day = hour.checked_mul(60)?.checked_add(minute)?;
    Some((days, minute_of_day))
}

pub(crate) fn parse_time_of_day(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    if !matches!(bytes.len(), 5 | 8) || bytes.get(2) != Some(&b':') {
        return None;
    }
    if bytes.len() == 8 && bytes.get(5) != Some(&b':') {
        return None;
    }
    let hour = decimal(bytes.get(0..2)?)?;
    let minute = decimal(bytes.get(3..5)?)?;
    let second = if bytes.len() == 8 {
        decimal(bytes.get(6..8)?)?
    } else {
        0
    };
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    hour.checked_mul(60)?.checked_add(minute)
}

fn duration_number(bytes: &[u8], start: &mut usize) -> Option<i64> {
    let number_start = *start;
    while bytes.get(*start).is_some_and(u8::is_ascii_digit) {
        *start = start.checked_add(1)?;
    }
    if *start == number_start {
        return None;
    }
    bytes
        .get(number_start..*start)?
        .iter()
        .try_fold(0i64, |value, byte| {
            let digit = i64::from(byte.checked_sub(b'0')?);
            value.checked_mul(10)?.checked_add(digit)
        })
}

pub(crate) fn parse_duration_minutes(value: &str) -> Option<(i64, bool)> {
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'P') {
        return None;
    }

    let mut index = 1usize;
    let mut days = 0i64;
    let mut saw_component = false;
    if bytes.get(index) != Some(&b'T') {
        days = duration_number(bytes, &mut index)?;
        saw_component = true;
        if bytes.get(index) != Some(&b'D') {
            return None;
        }
        index = index.checked_add(1)?;
    }
    if bytes.get(index) != Some(&b'T') {
        return None;
    }
    index = index.checked_add(1)?;

    let mut hours = 0i64;
    let mut minutes = 0i64;
    let mut seconds = 0i64;
    let mut last_rank = 0u8;
    while index < bytes.len() {
        let number = duration_number(bytes, &mut index)?;
        let (rank, slot) = match bytes.get(index)? {
            b'H' => (1, &mut hours),
            b'M' => (2, &mut minutes),
            b'S' => (3, &mut seconds),
            _ => return None,
        };
        if rank <= last_rank {
            return None;
        }
        *slot = number;
        last_rank = rank;
        saw_component = true;
        index = index.checked_add(1)?;
    }
    if !saw_component {
        return None;
    }

    let day_minutes = days.checked_mul(24)?.checked_mul(60)?;
    let hour_minutes = hours.checked_mul(60)?;
    let total = day_minutes
        .checked_add(hour_minutes)?
        .checked_add(minutes)?
        .checked_add(seconds / 60)?;
    Some((total, seconds % 60 != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_format_as_a_stamp_and_round_trip_through_days_from_civil() {
        assert_eq!(format_minutes(0), "1970-01-01 00:00");
        assert_eq!(format_minutes(-1), "1969-12-31 23:59");
        for (year, month, day) in [(2024, 2, 29), (2000, 1, 3), (2026, 10, 14), (1999, 12, 31)] {
            let minutes = days_from_civil(year, month, day) * 1_440 + 8 * 60 + 5;
            assert_eq!(
                format_minutes(minutes),
                format!("{year:04}-{month:02}-{day:02} 08:05")
            );
        }
        assert!(format_minutes(i64::MAX).len() > 10);
        assert!(format_minutes(i64::MIN).len() > 10);
    }

    #[test]
    fn known_dates_and_weekdays() {
        let monday = days_from_civil(2026, 1, 5);

        assert_eq!(weekday_of(monday), 0);
        assert_eq!(weekday_of(days_from_civil(1970, 1, 1)), 3);
        assert_eq!(parse_datetime("2026-01-05"), Some((monday, 0)));
        assert_eq!(parse_datetime("2026-01-05T08:30:59"), Some((monday, 510)));
    }

    #[test]
    fn accepts_leap_day_and_supported_suffixes() {
        let leap_day = days_from_civil(2024, 2, 29);

        assert_eq!(parse_datetime("2024-02-29"), Some((leap_day, 0)));
        assert_eq!(
            parse_datetime("2024-02-29T01:02:03.0Z"),
            Some((leap_day, 62))
        );
        assert_eq!(
            parse_datetime("2024-02-29T01:02:03+07:30"),
            Some((leap_day, 62))
        );
    }

    #[test]
    fn rejects_bad_dates() {
        for value in [
            "2023-02-29",
            "2024-13-01",
            "2024-01-00",
            "2024-01-01T24:00:00",
            "2024-01-01T00:60:00",
            "2024-01-01T00:00:60",
            "not a date",
        ] {
            assert_eq!(parse_datetime(value), None, "{value}");
        }
    }

    #[test]
    fn parses_times_of_day() {
        assert_eq!(parse_time_of_day("08:30:59"), Some(510));
        assert_eq!(parse_time_of_day("23:59"), Some(1439));
        assert_eq!(parse_time_of_day("24:00:00"), None);
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration_minutes("PT8H0M0S"), Some((480, false)));
        assert_eq!(parse_duration_minutes("PT0S"), Some((0, false)));
        assert_eq!(parse_duration_minutes("PT90M"), Some((90, false)));
        assert_eq!(parse_duration_minutes("PT1H30M30S"), Some((90, true)));
        assert_eq!(parse_duration_minutes("P1DT1H"), Some((1500, false)));
        assert_eq!(parse_duration_minutes("P0DT"), Some((0, false)));
    }

    #[test]
    fn rejects_bad_or_overflowing_durations() {
        for value in [
            "garbage",
            "PT",
            "PT-1H",
            "PT1.5H",
            "PT1M1H",
            "PT999999999999999999999999999999999999999999999999H",
        ] {
            assert_eq!(parse_duration_minutes(value), None, "{value}");
        }
    }
}
