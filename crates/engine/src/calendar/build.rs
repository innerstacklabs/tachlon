//! Building a [`Calendar`]: validation and the exception tables.

use super::index::{bad_period, bucket_index, joined, minutes_before};
use super::{
    Calendar, CalendarError, DAY, DayPeriod, ExceptionDay, MAX_EXCEPTION_DAY, TABLE_DAYS, WEEK,
};

impl Calendar {
    /// Build a calendar from per-weekday periods (index 0 = Monday) and
    /// holiday day indices (day `d` is `[d * 1440, (d + 1) * 1440)`). A
    /// holiday given twice counts once. The same as
    /// [`Calendar::with_exceptions`] with no periods on each holiday.
    ///
    /// # Errors
    ///
    /// Returns a [`CalendarError`] if a day's periods are empty, out of range, unsorted or
    /// overlapping, if no weekday has a working minute, or if a holiday is before the origin or
    /// after the last supported day.
    pub fn new(week: [Vec<DayPeriod>; 7], holidays: &[i64]) -> Result<Calendar, CalendarError> {
        let mut days = holidays.to_vec();
        days.sort_unstable();
        days.dedup();
        let exceptions: Vec<(i64, Vec<DayPeriod>)> =
            days.into_iter().map(|d| (d, Vec::new())).collect();
        Calendar::with_exceptions(week, &exceptions)
    }

    /// Build a calendar from per-weekday periods (index 0 = Monday) and
    /// exception days: `(day, periods)`, where the periods replace the
    /// weekday's periods on that one day. No periods is a holiday. The
    /// exceptions may come in any order; two on one day are refused with
    /// [`CalendarError::DuplicateException`].
    ///
    /// # Errors
    ///
    /// Returns a [`CalendarError`] for a bad weekday or exception period (`BadPeriod`,
    /// `BadException`), a week with no working minute (`NoWorkingTime`), an exception day before the
    /// origin or after the last supported day (`NegativeException`, `ExceptionTooLate`), or two
    /// exceptions on one day (`DuplicateException`).
    #[expect(clippy::too_many_lines, reason = "week and day tables in one pass")]
    #[expect(
        clippy::cast_possible_wrap,
        reason = "the tail_removed check casts day_table.len(), at most TABLE_DAYS = 36600"
    )]
    #[expect(
        clippy::missing_panics_doc,
        reason = "removed starts as vec![0] and only grows, so its last() is always Some"
    )]
    pub fn with_exceptions(
        week: [Vec<DayPeriod>; 7],
        exceptions: &[(i64, Vec<DayPeriod>)],
    ) -> Result<Calendar, CalendarError> {
        for (weekday, periods) in week.iter().enumerate() {
            if let Some(period) = bad_period(periods) {
                return Err(CalendarError::BadPeriod { weekday, period });
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "WEEK is the constant 10080"
        )]
        let mut working = vec![false; WEEK as usize];
        #[expect(clippy::cast_possible_truncation, reason = "DAY is the constant 1440")]
        for (weekday, periods) in week.iter().enumerate() {
            for &(s, e) in periods {
                for m in s..e {
                    working[weekday * DAY as usize + m as usize] = true;
                }
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "WEEK is the constant 10080"
        )]
        let mut cum_week = Vec::with_capacity(WEEK as usize + 1);
        let mut pos_week = Vec::new();
        cum_week.push(0u32);
        for (o, &w) in working.iter().enumerate() {
            if w {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "o is a minute of the week, below WEEK = 10080"
                )]
                pos_week.push(o as u32);
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a week has at most WEEK = 10080 working minutes"
            )]
            cum_week.push(pos_week.len() as u32);
        }
        #[expect(
            clippy::cast_possible_wrap,
            reason = "a week has at most WEEK = 10080 working minutes"
        )]
        let weekly = pos_week.len() as i64;
        if weekly == 0 {
            return Err(CalendarError::NoWorkingTime);
        }
        for (day, periods) in exceptions {
            if *day < 0 {
                return Err(CalendarError::NegativeException(*day));
            }
            if *day > MAX_EXCEPTION_DAY {
                return Err(CalendarError::ExceptionTooLate(*day));
            }
            if let Some(period) = bad_period(periods) {
                return Err(CalendarError::BadException { day: *day, period });
            }
        }
        let mut exceptions = exceptions.to_vec();
        exceptions.sort_by_key(|&(day, _)| day);
        if let Some(pair) = exceptions.windows(2).find(|p| p[0].0 == p[1].0) {
            return Err(CalendarError::DuplicateException(pair[0].0));
        }
        let holiday_days = exceptions
            .iter()
            .filter(|(_, periods)| periods.is_empty())
            .map(|&(day, _)| day)
            .collect();
        let mut cal = Calendar {
            week,
            exceptions,
            holiday_days,
            cum_week,
            pos_week,
            weekly,
            exdays: Vec::new(),
            removed: vec![0],
            key: Vec::new(),
            by_week: Vec::new(),
            by_key: Vec::new(),
            day_table: Vec::new(),
            tail_removed: None,
        };
        let (mut exdays, mut removed, mut key) = (Vec::new(), vec![0i64], Vec::new());
        for (src, (day, periods)) in cal.exceptions.iter().enumerate() {
            #[expect(
                clippy::cast_sign_loss,
                reason = "day is non-negative (a negative exception day is refused above), so day \
                          % 7 is in 0..7"
            )]
            if joined(periods) == joined(&cal.week[(day % 7) as usize]) {
                continue; // the pattern's own minutes (a holiday on a non-working day)
            }
            let first = cal.pattern_count(day * DAY);
            let pattern = cal.pattern_count((day + 1) * DAY) - first;
            let minutes = minutes_before(periods, DAY);
            let before = *removed.last().expect("removed starts non-empty");
            key.push(first - before);
            exdays.push(ExceptionDay {
                day: *day,
                minutes,
                src,
            });
            removed.push(before + pattern - minutes);
        }
        (cal.exdays, cal.removed, cal.key) = (exdays, removed, key);
        cal.by_week = bucket_index(cal.exdays.iter().map(|x| x.day), 7);
        cal.by_key = bucket_index(cal.key.iter().copied(), weekly);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_possible_wrap,
            reason = "x.day is non-negative (a negative exception day is refused above) and the \
                      min caps it at TABLE_DAYS = 36600"
        )]
        let table_len = cal
            .exdays
            .last()
            .map_or(0, |x| (x.day + 2).min(TABLE_DAYS as i64) as usize);
        cal.day_table.reserve(table_len);
        let mut n = 0usize;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_lossless,
            reason = "table_len and n are at most TABLE_DAYS = 36600; |removed| is at most \
                      TABLE_DAYS * DAY, which the const assert in calendar.rs holds below i32::MAX \
                      >> 1; i32 widens to i64 exactly"
        )]
        for day in 0..table_len as i64 {
            if n < cal.exdays.len() && cal.exdays[n].day == day {
                cal.day_table.push((n as i32) << 1 | 1);
                n += 1;
            } else {
                let removed = cal.removed[n];
                debug_assert!(removed.abs() <= (i32::MAX >> 1) as i64);
                cal.day_table.push((removed as i32) << 1);
            }
        }
        cal.tail_removed = cal
            .exdays
            .iter()
            .all(|x| x.day < cal.day_table.len() as i64)
            .then(|| *cal.removed.last().expect("removed starts non-empty"));
        Ok(cal)
    }
}
