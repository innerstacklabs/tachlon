//! The schedule's calendars as MSPDI `Calendar` elements: the seven weekdays with their working
//! times, then one `DayType` 0 entry for each exception day. Each is a base calendar with no
//! parent, so the reader gets the same week and the same exceptions back. A calendar's `UID` is
//! its position in the schedule plus one.

use engine::{Calendar, Schedule};

use super::xml::{Xml, date_text, time_text};
use crate::ImportError;

/// The `DayType` of MSPDI weekdays 1 (Sunday) to 7 (Saturday), as an index into the engine's
/// Monday-first week.
fn week_index(day_type: u8) -> usize {
    usize::from((day_type + 5) % 7)
}

fn working_times(xml: &mut Xml, periods: &[(u16, u16)]) {
    if periods.is_empty() {
        return;
    }
    xml.open("WorkingTimes");
    for &(from, to) in periods {
        xml.open("WorkingTime");
        xml.leaf("FromTime", &time_text(from));
        xml.leaf("ToTime", &time_text(to));
        xml.close("WorkingTime");
    }
    xml.close("WorkingTimes");
}

fn write_day(xml: &mut Xml, day_type: u8, periods: &[(u16, u16)]) {
    xml.open("WeekDay");
    xml.leaf("DayType", &day_type.to_string());
    xml.leaf("DayWorking", if periods.is_empty() { "0" } else { "1" });
    working_times(xml, periods);
    xml.close("WeekDay");
}

fn write_calendar(
    xml: &mut Xml,
    uid: usize,
    calendar: &Calendar,
    origin_day: i64,
) -> Result<(), ImportError> {
    xml.open("Calendar");
    xml.leaf("UID", &uid.to_string());
    xml.leaf("Name", &format!("Calendar {uid}"));
    xml.leaf("IsBaseCalendar", "1");
    xml.leaf("BaseCalendarUID", "-1");
    xml.open("WeekDays");
    for day_type in 1..=7u8 {
        let periods = calendar
            .week()
            .get(week_index(day_type))
            .map_or(&[][..], Vec::as_slice);
        write_day(xml, day_type, periods);
    }
    for (day, periods) in calendar.exceptions() {
        let date = date_text(origin_day.saturating_add(*day), "FromDate")?;
        xml.open("WeekDay");
        xml.leaf("DayType", "0");
        xml.leaf("DayWorking", if periods.is_empty() { "0" } else { "1" });
        xml.open("TimePeriod");
        xml.leaf("FromDate", &format!("{date}T00:00:00"));
        xml.leaf("ToDate", &format!("{date}T23:59:00"));
        xml.close("TimePeriod");
        working_times(xml, periods);
        xml.close("WeekDay");
    }
    xml.close("WeekDays");
    xml.close("Calendar");
    Ok(())
}

/// Write the `Calendars` element, or nothing for a schedule with no calendar.
pub(super) fn write(
    xml: &mut Xml,
    schedule: &Schedule,
    origin_day: i64,
) -> Result<(), ImportError> {
    if schedule.calendars().is_empty() {
        return Ok(());
    }
    xml.open("Calendars");
    for (index, calendar) in schedule.calendars().iter().enumerate() {
        write_calendar(xml, index + 1, calendar, origin_day)?;
    }
    xml.close("Calendars");
    Ok(())
}
