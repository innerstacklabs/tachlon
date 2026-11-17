//! Working calendars and the one primitive built on them, [`Calendar::add`].
//!
//! Time model (plan D5): an [`Instant`] is an integer count of wall-clock
//! minutes from the project origin. The origin is instant 0 and is a
//! **Monday 00:00**; there are no time zones and no DST. Minute `t` is the
//! half-open span `[t, t + 1)`; "instant `t` is working" means that minute is
//! working.
//!
//! A calendar is a weekly pattern of working periods plus a list of exception
//! days. An exception day's periods replace the pattern's periods for that one
//! date: none is a holiday, fewer minutes a shortened day, and periods on a
//! day the pattern does not work (a working Saturday) add time. Internally it
//! is reduced to `C(t)`, the number of working minutes in `[0, t)`, and its
//! index inverse `nth(k)`, the instant of the `k`-th working minute (0-based).
//! `count` and `is_working` read a dense per-day table for the first 100
//! years, falling back to a per-week index and short scan beyond it. `nth`
//! uses a separate index over working-minute keys.
//!
//! Each exception day carries a signed offset: the pattern minutes of that day
//! minus its exception minutes, negative when the exception adds time. Off an
//! exception day, `C(t)` is the pattern count less the sum of the offsets of
//! the exceptions before it; on one, it is `C` at the day's start plus the
//! exception's minutes before `t`.

mod build;
mod error;
mod index;
mod query;

pub use error::CalendarError;
use index::{first_not_before, minutes_before};

/// Wall-clock minutes from the project origin (Monday 00:00).
pub type Instant = i64;
/// A count of working minutes in some calendar.
pub type Minutes = i64;

/// Minutes in a day.
pub const DAY: i64 = 1440;
/// Minutes in a week.
pub const WEEK: i64 = 7 * DAY;
/// The last day an exception may fall on: the end of that day,
/// `(day + 1) * DAY`, still fits in an [`Instant`].
pub const MAX_EXCEPTION_DAY: i64 = i64::MAX / DAY - 1;

/// A working period within one day, in minutes from that day's midnight:
/// `[start, end)` with `0 <= start < end <= 1440`.
pub type DayPeriod = (u16, u16);

/// An exception day whose working minutes differ from the pattern's.
#[derive(Debug, Clone)]
struct ExceptionDay {
    /// Day index from the origin.
    day: i64,
    /// Working minutes of the exception on this day.
    minutes: i64,
    /// Index of the exception in `Calendar::exceptions`.
    src: usize,
}

/// A weekly working pattern with per-date exception days.
#[derive(Debug, Clone)]
pub struct Calendar {
    week: [Vec<DayPeriod>; 7],
    /// Every exception as given, sorted by day.
    exceptions: Vec<(i64, Vec<DayPeriod>)>,
    /// The exception days with no periods.
    holiday_days: Vec<i64>,
    /// `cum_week[o]` = working minutes in `[0, o)` of a week, `o` in `0..=WEEK`.
    cum_week: Vec<u32>,
    /// `pos_week[r]` = week offset of the `r`-th working minute of the week.
    pos_week: Vec<u32>,
    /// Working minutes per pattern week.
    weekly: i64,
    /// The exception days that change the pattern's minutes, sorted by day.
    /// An exception identical to its weekday's pattern is left out.
    exdays: Vec<ExceptionDay>,
    /// `removed[i]` = the sum over the first `i` exception days of (pattern
    /// minutes - exception minutes) of the day. Signed: an exception that
    /// adds time (a working Saturday) makes it smaller.
    removed: Vec<i64>,
    /// `key[i]` = `C` at the start of exception day `i` (its pattern count less
    /// `removed[i]`), non-decreasing; used by `nth`.
    key: Vec<i64>,
    /// `by_week[w]` = exception days before `7 * w` (the first exception not
    /// before week `w`); covers weeks up to the last exception's, capped at
    /// [`INDEX_CAP`] entries.
    by_week: Vec<u32>,
    /// `by_key[b]` = keys below `b * weekly` (the first key not before working
    /// week `b`); same coverage and cap.
    by_key: Vec<u32>,
    /// Per covered day, the low bit is the exception flag. Off an exception
    /// day the entry is `removed before day << 1`; on one it is
    /// `exception index << 1 | 1`.
    day_table: Vec<i32>,
    /// Total removed minutes when every exception is covered by `day_table`.
    tail_removed: Option<i64>,
}

/// Most entries per exception index (about 1250 years of weeks); an
/// exception past it falls back to a binary search over the exceptions beyond
/// the index.
const INDEX_CAP: usize = 1 << 16;

/// Days covered by the dense exception table (100 leap years,
/// conservatively): about 36,600 entries of 4 bytes, or 146,400 bytes per
/// calendar.
const TABLE_DAYS: usize = 100 * 366;

// A table entry holds a signed count of removed minutes, or an exception
// index, shifted left by one. Each covered day adds or removes at most `DAY`
// minutes, so the count stays within `TABLE_DAYS * DAY` either way.
#[expect(
    clippy::cast_possible_wrap,
    reason = "TABLE_DAYS is the constant 36600"
)]
const _: () = assert!(TABLE_DAYS as i64 * DAY <= (i32::MAX >> 1) as i64);

impl Calendar {
    /// Working periods per weekday, as given to [`Calendar::new`] or
    /// [`Calendar::with_exceptions`].
    #[must_use]
    pub fn week(&self) -> &[Vec<DayPeriod>; 7] {
        &self.week
    }

    /// Every exception day as given, sorted by day: `(day, periods)`, where no
    /// periods is a holiday.
    #[must_use]
    pub fn exceptions(&self) -> &[(i64, Vec<DayPeriod>)] {
        &self.exceptions
    }

    /// Holiday day indices, sorted: the exception days with no periods.
    #[must_use]
    pub fn holidays(&self) -> &[i64] {
        &self.holiday_days
    }

    /// Working minutes per week of the pattern, before exceptions.
    #[must_use]
    pub fn weekly_minutes(&self) -> i64 {
        self.weekly
    }

    /// Pattern working minutes (exceptions ignored) in `[0, t)`, `t >= 0`.
    #[expect(
        clippy::cast_lossless,
        clippy::cast_sign_loss,
        reason = "u32 widens to i64 exactly, and t >= 0 (documented; count asserts it in debug \
                  builds), so t % WEEK is in 0..WEEK"
    )]
    fn pattern_count(&self, t: Instant) -> i64 {
        (t / WEEK) * self.weekly + self.cum_week[(t % WEEK) as usize] as i64
    }

    /// Instant of the `j`-th pattern working minute (exceptions ignored).
    #[expect(
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "u32 widens to i64 exactly; j is a non-negative pattern minute and j % weekly is \
                  below weekly, at most 10080"
    )]
    fn pattern_instant(&self, j: i64) -> Instant {
        (j / self.weekly) * WEEK + self.pos_week[(j % self.weekly) as usize] as i64
    }

    /// How many exception days fall before `day` (`day >= 0`).
    fn exception_index(&self, day: i64) -> usize {
        first_not_before(&self.exdays, &self.by_week, day / 7, |x| x.day < day)
    }

    /// The periods of exception day `n`.
    fn exception_periods(&self, n: usize) -> &[DayPeriod] {
        &self.exceptions[self.exdays[n].src].1
    }

    /// `C(t)` for an instant on exception day `n`: `C` at the day's start plus
    /// the exception's minutes before `t`.
    fn exception_count(&self, n: usize, t: Instant) -> Minutes {
        let start = self.exdays[n].day * DAY;
        self.key[n] + minutes_before(self.exception_periods(n), t - start)
    }

    /// Is `t`, on exception day `n`, inside one of the exception's periods?
    #[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
    fn exception_working(&self, n: usize, t: Instant) -> bool {
        let m = t - self.exdays[n].day * DAY;
        self.exception_periods(n)
            .iter()
            .any(|&(s, e)| s as i64 <= m && m < e as i64)
    }
}

#[cfg(test)]
mod tests;
