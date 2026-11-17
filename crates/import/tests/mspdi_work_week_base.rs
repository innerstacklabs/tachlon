//! A derived calendar's work week keeps the holidays its base calendar lists.

use import::parse_mspdi;

const TIMES: &str =
    "<WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>";

/// Every weekday of a week, Monday to Friday worked 08:00 to 16:00 (`DayType` 2 to 6).
fn monday_to_friday() -> String {
    (1..=7u8)
        .map(|day| {
            if (2..=6).contains(&day) {
                format!(
                    "<WeekDay><DayType>{day}</DayType><DayWorking>1</DayWorking>\
<WorkingTimes>{TIMES}</WorkingTimes></WeekDay>"
                )
            } else {
                format!("<WeekDay><DayType>{day}</DayType><DayWorking>0</DayWorking></WeekDay>")
            }
        })
        .collect()
}

/// A work week over January 2026 that works Monday to Saturday.
fn saturday_work_week() -> String {
    let days: String = (2..=7u8)
        .map(|day| {
            format!(
                "<WeekDay><DayType>{day}</DayType><DayWorking>1</DayWorking>\
<WorkingTimes>{TIMES}</WorkingTimes></WeekDay>"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "<WorkWeek><TimePeriod><FromDate>2026-01-01T00:00:00</FromDate>\
<ToDate>2026-01-31T23:59:00</ToDate></TimePeriod><Name>Saturdays</Name>\
<WeekDays>{days}</WeekDays></WorkWeek>"
    )
}

/// A one-day exception on `date`, a holiday or a working day.
fn day_exception(date: &str, working: bool) -> String {
    let times = if working {
        format!("<WorkingTimes>{TIMES}</WorkingTimes>")
    } else {
        String::new()
    };
    format!(
        "<Exception><TimePeriod><FromDate>{date}T00:00:00</FromDate>\
<ToDate>{date}T23:59:00</ToDate></TimePeriod><DayWorking>{}</DayWorking>{times}</Exception>",
        u8::from(working)
    )
}

/// The finish minute, from Monday 2026-01-05 00:00, of a 48-hour task on calendar 2. Calendar 1
/// is the Monday to Friday base with `base_exception`; calendar 2 is based on it, works Monday
/// to Saturday in January through a work week, and has `own_exception`.
fn finish_on_derived(base_exception: &str, own_exception: &str) -> i64 {
    let xml = format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-05T08:00:00</StartDate><CalendarUID>2</CalendarUID><Calendars>\
<Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar>\
<BaseCalendarUID>-1</BaseCalendarUID><WeekDays>{}</WeekDays>\
<Exceptions>{base_exception}</Exceptions></Calendar>\
<Calendar><UID>2</UID><Name>Saturdays</Name><IsBaseCalendar>0</IsBaseCalendar>\
<BaseCalendarUID>1</BaseCalendarUID><Exceptions>{own_exception}</Exceptions>\
<WorkWeeks>{}</WorkWeeks></Calendar></Calendars>\
<Tasks><Task><UID>1</UID><Name>T</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT48H0M0S</Duration><DurationFormat>7</DurationFormat><CalendarUID>2</CalendarUID>\
</Task></Tasks></Project>",
        monday_to_friday(),
        saturday_work_week(),
    );
    let mut imported = parse_mspdi(&xml).expect("derived calendar project should import");
    let id = engine::TaskId(0);
    imported
        .schedule
        .compute()
        .expect("derived calendar project should compute")
        .get(id)
        .ef
}

const DAY: i64 = 1440;
const FOUR: i64 = 16 * 60;

#[test]
fn a_work_week_keeps_the_holiday_its_base_calendar_lists() {
    // Six 8-hour days on Monday to Saturday from Monday 2026-01-05, with the base calendar's
    // holiday on Wednesday 01-07: Mon 05, Tue 06, Thu 08, Fri 09, Sat 10, then Mon 12 at 16:00,
    // seven days after the start day.
    let finish = finish_on_derived(&day_exception("2026-01-07", false), "");
    assert_eq!(finish, 7 * DAY + FOUR);
}

#[test]
fn a_base_holiday_on_a_saturday_the_work_week_works_stays_a_holiday() {
    // The base holiday is Saturday 2026-01-10, a day the work week lists as worked: Mon 05 to
    // Fri 09 are five days, Sat 10 is off, so the sixth day is Mon 12 at 16:00.
    let finish = finish_on_derived(&day_exception("2026-01-10", false), "");
    assert_eq!(finish, 7 * DAY + FOUR);
}

#[test]
fn a_derived_calendars_own_exception_overrides_one_base_holiday_and_the_other_stays() {
    // The base calendar has holidays on Wednesday 01-07 and Thursday 01-08. The derived
    // calendar's own working exception on Wednesday 07 beats that holiday, and Thursday stays a
    // holiday: Mon 05, Tue 06, Wed 07, Fri 09, Sat 10, then Mon 12 at 16:00.
    let base = format!(
        "{}{}",
        day_exception("2026-01-07", false),
        day_exception("2026-01-08", false)
    );
    let finish = finish_on_derived(&base, &day_exception("2026-01-07", true));
    assert_eq!(finish, 7 * DAY + FOUR);
}
