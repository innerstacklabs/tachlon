//! MSPDI calendar exceptions: one-off holidays (`DayWorking=0`) and working days
//! (`DayWorking=1`, with their own `WorkingTimes`), recurring exceptions and work weeks,
//! each as `(day, periods)`.

use super::{keep_valid_periods, weekday_name, working_periods};
use crate::ImportError;
use crate::calendar_rule::Exception;
use crate::dom::Node;
use crate::fields::{flag, invalid, optional_u32};
use crate::notes::Notes;
use crate::units::{days_from_civil, parse_datetime, weekday_of};

/// The most days (or occurrences) one exception or work week expands to.
const MOST_DAYS: i64 = 3660;

/// How an exception element repeats, read from its `Type` and the fields that type uses.
enum Shape {
    /// One date range: every date from `FromDate` to `ToDate` gets the same periods.
    Range,
    /// A repeating pattern between `FromDate` and `ToDate`.
    Repeats(Recurrence),
    /// A pattern this reader does not expand.
    Unsupported,
}

enum Recurrence {
    /// Every `every` days from `FromDate`.
    Daily { every: i64 },
    /// The days in `mask` (Sunday is bit 0) of every `every`th week, weeks starting on Sunday.
    Weekly { every: i64, mask: u32 },
    /// `day` of every `every`th month, the last day of a shorter month.
    Monthly { every: i64, day: u32 },
    /// `day` of `month` (1 to 12) each year, the last day of a shorter month.
    Yearly { month: u32, day: u32 },
}

/// MS Project writes recurring exceptions as one element whose `TimePeriod` spans the whole
/// recurrence window and whose `Type` names the pattern: 1 or 7 daily, 2 yearly on a date,
/// 3 yearly on a weekday of a month, 4 monthly on a date, 5 monthly on a weekday, 6 weekly.
/// The meanings follow the MPXJ reader. A daily pattern every day is one plain range, and the
/// relative patterns (types 3 and 5) are not expanded.
fn shape(node: &Node) -> Result<Shape, ImportError> {
    let by_occurrences = flag(node, "EnteredByOccurrences")? == Some(true);
    let counted = optional_u32(node, "Occurrences")?.is_some_and(|count| count > 1);
    let every = i64::from(optional_u32(node, "Period")?.unwrap_or(1).max(1));
    let month_day = optional_u32(node, "MonthDay")?.filter(|day| (1..=31).contains(day));
    Ok(match optional_u32(node, "Type")? {
        None if (by_occurrences && counted) || every > 1 => Shape::Unsupported,
        None => Shape::Range,
        Some(1 | 7) if every == 1 => Shape::Range,
        Some(1 | 7) => Shape::Repeats(Recurrence::Daily { every }),
        Some(6) => match optional_u32(node, "DaysOfWeek")?.map(|bits| bits & 0x7f) {
            Some(mask) if mask != 0 => Shape::Repeats(Recurrence::Weekly { every, mask }),
            _ => Shape::Unsupported,
        },
        Some(4) => month_day.map_or(Shape::Unsupported, |day| {
            Shape::Repeats(Recurrence::Monthly { every, day })
        }),
        Some(2) => match (optional_u32(node, "Month")?, month_day) {
            // Month counts from 0 (January).
            (Some(month @ 0..=11), Some(day)) => Shape::Repeats(Recurrence::Yearly {
                month: month + 1,
                day,
            }),
            _ => Shape::Unsupported,
        },
        Some(_) => Shape::Unsupported,
    })
}

/// The days (since 1970-01-01) from `year-month` onward where `day` falls, clamped to the
/// month's length, for a pattern that repeats by month.
fn day_of_month(year: i32, month: u32, day: u32) -> i64 {
    let first = days_from_civil(year, month, 1);
    let next = if month == 12 {
        days_from_civil(year + 1, 1, 1)
    } else {
        days_from_civil(year, month + 1, 1)
    };
    first + i64::from(day).min(next - first) - 1
}

/// Every date of `recurrence` from `from` to `to`, both days since 1970-01-01, at most
/// `MOST_DAYS + 1` of them. `year` and `month` are those of `from`.
fn occurrences(recurrence: &Recurrence, from: i64, to: i64, year: i32, month: u32) -> Vec<i64> {
    let mut dates = Vec::new();
    let mut add = |date: i64| {
        if (from..=to).contains(&date) {
            dates.push(date);
        }
        date <= to && dates.len() <= usize::try_from(MOST_DAYS).unwrap_or(usize::MAX)
    };
    match *recurrence {
        Recurrence::Daily { every } => {
            let mut date = from;
            while add(date) {
                date += every;
            }
        }
        Recurrence::Weekly { every, mask } => {
            // Weeks start on Sunday; the week of `from` counts as the first.
            let mut sunday = from - i64::from((weekday_of(from) + 1) % 7);
            'weeks: while sunday <= to {
                for offset in 0..7 {
                    if mask & (1 << offset) != 0 && !add(sunday + i64::from(offset)) {
                        break 'weeks;
                    }
                }
                sunday += 7 * every;
            }
        }
        Recurrence::Monthly { every, day } => {
            let mut index = i64::from(year) * 12 + i64::from(month) - 1;
            loop {
                let (y, m) = (index.div_euclid(12), index.rem_euclid(12));
                let (Ok(y), Ok(m)) = (i32::try_from(y), u32::try_from(m)) else {
                    break;
                };
                let date = day_of_month(y, m + 1, day);
                if date >= from && !add(date) {
                    break;
                }
                index += every;
            }
        }
        Recurrence::Yearly { month, day } => {
            let mut y = year;
            loop {
                let date = day_of_month(y, month, day);
                if date >= from && !add(date) {
                    break;
                }
                y += 1;
            }
        }
    }
    dates
}

/// The exception days one `Exception` or `WeekDay` (type 0) element describes, each with
/// the periods that apply on it. A range gets the same periods on every date.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pass over an element's days; splitting it would only move the same locals into arguments"
)]
pub(super) fn exception_days(
    node: &Node,
    uid: u32,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<Vec<Exception>, ImportError> {
    let shape = shape(node)?;
    if matches!(shape, Shape::Unsupported) {
        notes.line(format!(
            "Calendar {uid}: a recurring exception was not read; working days can differ from the file's."
        ));
        return Ok(Vec::new());
    }

    let (periods, unreadable) = match flag(node, "DayWorking")? {
        Some(true) => {
            let (periods, unreadable) = working_periods(node);
            if periods.is_empty() && unreadable == 0 {
                // Empty periods would read as a holiday, the opposite of a working day.
                notes.line(format!(
                    "Calendar {uid}: a working exception with no working times was not read; those days keep the calendar's usual week."
                ));
                return Ok(Vec::new());
            }
            (periods, unreadable)
        }
        Some(false) => (Vec::new(), 0),
        None => {
            notes.line(format!(
                "Calendar {uid}: an exception that does not say whether the day is worked was not read; those days keep the calendar's usual week."
            ));
            return Ok(Vec::new());
        }
    };

    let Some(period) = node.child("TimePeriod") else {
        notes.line(format!(
            "Calendar {uid}: an exception with no dates was not read; those days keep the calendar's usual week."
        ));
        return Ok(Vec::new());
    };
    let Some(from_value) = period.child_text("FromDate") else {
        notes.line(format!(
            "Calendar {uid}: an exception with no first date was not read; those days keep the calendar's usual week."
        ));
        return Ok(Vec::new());
    };
    let Some(to_value) = period.child_text("ToDate") else {
        notes.line(format!(
            "Calendar {uid}: an exception with no last date was not read; those days keep the calendar's usual week."
        ));
        return Ok(Vec::new());
    };
    // Unreadable or overlapping periods on one exception drop with a note, not a failed import.
    let was_working = !periods.is_empty() || unreadable > 0;
    let date = |value: &str| value.split('T').next().unwrap_or(value).to_owned();
    let day = if date(from_value) == date(to_value) {
        date(from_value)
    } else {
        format!("{} to {}", date(from_value), date(to_value))
    };
    let periods = keep_valid_periods(periods, unreadable, uid, &day, notes);
    if was_working && periods.is_empty() {
        // Every period was refused; an empty list would read as a holiday.
        notes.line(format!(
            "Calendar {uid}: a working exception whose working times are all invalid was not read; those days keep the calendar's usual week."
        ));
        return Ok(Vec::new());
    }
    let (from_day, _) =
        parse_datetime(from_value).ok_or_else(|| invalid("FromDate", from_value))?;
    let (to_day, _) = parse_datetime(to_value).ok_or_else(|| invalid("ToDate", to_value))?;
    let bad_period = || invalid("TimePeriod", &format!("{from_value} to {to_value}"));
    let count = to_day
        .checked_sub(from_day)
        .and_then(|days| days.checked_add(1))
        .filter(|days| *days > 0)
        .ok_or_else(bad_period)?;

    let dates: Vec<i64> = if let Shape::Repeats(recurrence) = shape {
        let year = from_value.get(0..4).and_then(|text| text.parse().ok());
        let month = from_value.get(5..7).and_then(|text| text.parse().ok());
        let (Some(year), Some(month)) = (year, month) else {
            return Err(bad_period());
        };
        let mut dates = occurrences(&recurrence, from_day, to_day, year, month);
        if dates.len() > usize::try_from(MOST_DAYS).unwrap_or(usize::MAX) {
            dates.truncate(usize::try_from(MOST_DAYS).unwrap_or(usize::MAX));
            notes.line(format!(
                    "Calendar {uid}: an exception longer than 3660 days was cut to 3660 days; the days after that keep the calendar's usual week."
                ));
        }
        dates
    } else {
        if count > MOST_DAYS {
            notes.line(format!(
                    "Calendar {uid}: an exception longer than 3660 days was cut to 3660 days; the days after that keep the calendar's usual week."
                ));
        }
        (0..count.min(MOST_DAYS))
            .map(|offset| from_day.checked_add(offset).ok_or_else(bad_period))
            .collect::<Result<_, _>>()?
    };
    Ok(dates
        .into_iter()
        .filter_map(|day| {
            let relative = day.checked_sub(origin_day)?;
            (relative >= 0).then(|| (relative, periods.clone()))
        })
        .collect())
}

/// The days one `WorkWeek` element sets: for every date from its `FromDate` to its `ToDate`,
/// the periods the work week states for that weekday. A weekday it lists with `DayWorking`
/// 0 is not worked. A weekday it does not list uses the calendar's own week (or its base's),
/// as in MS Project, so that date gets no entry here. A week without a readable date range
/// is dropped with a note.
pub(super) fn work_week_days(
    node: &Node,
    uid: u32,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<Vec<Exception>, ImportError> {
    let dates = node.child("TimePeriod").and_then(|period| {
        let from = parse_datetime(period.child_text("FromDate")?)?.0;
        let to = parse_datetime(period.child_text("ToDate")?)?.0;
        Some((from, to))
    });
    let Some((from_day, to_day)) = dates.filter(|(from, to)| to >= from) else {
        notes.line(format!(
            "Calendar {uid}: a work week with no valid date range was not read; its dates keep the calendar's usual week."
        ));
        return Ok(Vec::new());
    };

    // Monday first, like the engine's week.
    // `None` is a weekday the work week does not list.
    let mut week: [Option<Vec<engine::DayPeriod>>; 7] = std::array::from_fn(|_| None);
    for week_day in node
        .child("WeekDays")
        .into_iter()
        .flat_map(|days| days.children_named("WeekDay"))
    {
        let day_value = week_day.child_text("DayType").unwrap_or_default();
        let day_type: u8 = day_value
            .parse()
            .ok()
            .filter(|day| (1..=7).contains(day))
            .ok_or_else(|| invalid("DayType", day_value))?;
        let kept = if flag(week_day, "DayWorking")? == Some(true) {
            let (periods, unreadable) = working_periods(week_day);
            keep_valid_periods(periods, unreadable, uid, weekday_name(day_type), notes)
        } else {
            Vec::new()
        };
        if let Some(slot) = week.get_mut(usize::from((day_type + 5) % 7)) {
            *slot = Some(kept);
        }
    }

    if to_day - from_day >= MOST_DAYS {
        notes.line(format!(
            "Calendar {uid}: a work week longer than 3660 days was cut to 3660 days; the days after that keep the calendar's usual week."
        ));
    }
    Ok((from_day..=to_day)
        .take(usize::try_from(MOST_DAYS).unwrap_or(usize::MAX))
        .filter(|day| *day >= origin_day)
        .filter_map(|day| {
            let periods = week.get(usize::from(weekday_of(day)))?.clone()?;
            Some((day - origin_day, periods))
        })
        .collect())
}
