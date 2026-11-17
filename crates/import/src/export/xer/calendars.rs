//! The schedule's calendars as the `CALENDAR` table of an XER file: one `CA_Base` row for each
//! calendar, holding its week and exception days as `clndr_data`. A calendar's `clndr_id` is its
//! position in the schedule plus one.

use engine::{Calendar, Schedule};
use xer::{CalendarData, CalendarException, Cell, Period, Week, Writer};

use crate::ImportError;

/// Days from the XER serial epoch, 1899-12-30, to 1970-01-01.
const EPOCH_TO_1970: i64 = 25_569;

const FIELDS: [&str; 5] = [
    "clndr_id",
    "clndr_name",
    "clndr_type",
    "day_hr_cnt",
    "clndr_data",
];

fn invalid(element: &str, value: &impl ToString) -> ImportError {
    ImportError::Invalid {
        element: element.to_owned(),
        value: value.to_string(),
    }
}

fn periods(engine: &[(u16, u16)]) -> Vec<Period> {
    engine
        .iter()
        .map(|&(start, finish)| Period { start, finish })
        .collect()
}

/// The engine's Monday-first week as an XER Sunday-first one.
fn week(calendar: &Calendar) -> Week {
    let monday_first = calendar.week();
    std::array::from_fn(|sunday_first| {
        let engine_index = (sunday_first + 6) % 7;
        monday_first
            .get(engine_index)
            .map_or_else(Vec::new, |day| periods(day))
    })
}

fn exceptions(calendar: &Calendar, origin_day: i64) -> Result<Vec<CalendarException>, ImportError> {
    calendar
        .exceptions()
        .iter()
        .map(|(day, working)| {
            let serial = origin_day
                .checked_add(*day)
                .and_then(|days| days.checked_add(EPOCH_TO_1970))
                .and_then(|serial| i32::try_from(serial).ok())
                .ok_or_else(|| invalid("exception date", &day))?;
            Ok(CalendarException {
                serial,
                periods: periods(working),
                unparsed_periods: 0,
            })
        })
        .collect()
}

/// The hours of a typical working day: the week's minutes over its working days, 8 for a week
/// with none.
#[expect(
    clippy::cast_precision_loss,
    reason = "a week holds at most 7 x 1440 minutes and has at most 7 working days, far below the 2^53 an f64 holds exactly"
)]
fn day_hours(calendar: &Calendar) -> f64 {
    let working_days = calendar.week().iter().filter(|day| !day.is_empty()).count();
    if working_days == 0 {
        return 8.0;
    }
    calendar.weekly_minutes() as f64 / working_days as f64 / 60.0
}

/// Write the `CALENDAR` table into `writer`: one row for each calendar of `schedule`.
/// `origin_day` is the day of the schedule's day 0, in days since 1970-01-01, as
/// [`Imported::origin_day`](crate::Imported::origin_day).
///
/// # Errors
///
/// [`ImportError::Invalid`] when an exception date does not fit an XER day count or a cell
/// cannot be written.
pub(super) fn write_calendars(
    writer: &mut Writer,
    schedule: &Schedule,
    origin_day: i64,
) -> Result<(), ImportError> {
    let cell_error = |error: xer::WriteError| invalid("CALENDAR", &error);
    writer.table("CALENDAR", &FIELDS).map_err(cell_error)?;
    for (index, calendar) in schedule.calendars().iter().enumerate() {
        let uid = index + 1;
        let data = CalendarData {
            week: Some(week(calendar)),
            exceptions: exceptions(calendar, origin_day)?,
        };
        let (id, name) = (uid.to_string(), format!("Calendar {uid}"));
        let hours = day_hours(calendar).to_string();
        let text = xer::write_clndr_data(&data);
        writer
            .row(&[
                Cell::Text(&id),
                Cell::Text(&name),
                Cell::Text("CA_Base"),
                Cell::Text(&hours),
                Cell::Text(&text),
            ])
            .map_err(cell_error)?;
    }
    Ok(())
}
