use engine::TaskId;
use import::{Imported, parse_mspdi};

fn task_id(imported: &Imported, uid: u32) -> TaskId {
    let position = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid)
        .expect("fixture task UID should be imported");
    TaskId(u32::try_from(position).expect("fixture task count should fit in u32"))
}

const WEEK: &str = "<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>";

/// A one-calendar Mon-Fri project with the given exceptions and task elements.
fn project(start: &str, exceptions: &str, tasks: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>{start}</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID>\
<Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{WEEK}</WeekDays><Exceptions>{exceptions}</Exceptions></Calendar></Calendars>\
<Tasks>{tasks}</Tasks></Project>"
    )
}

/// A working exception: the given periods replace the weekly pattern on `from` to `to`.
fn working_exception(from: &str, to: &str, times: &str) -> String {
    format!(
        "<Exception><TimePeriod><FromDate>{from}T00:00:00</FromDate><ToDate>{to}T23:59:00</ToDate>\
</TimePeriod><DayWorking>1</DayWorking><WorkingTimes>{times}</WorkingTimes></Exception>"
    )
}

type Applied = Vec<(i64, Vec<(u16, u16)>)>;

fn finish_of_six_working_days(exceptions: &str) -> (i64, Vec<String>, Applied) {
    let task = "<Task><UID>1</UID><Name>T</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT48H0M0S</Duration><DurationFormat>7</DurationFormat></Task>";
    let xml = project("2026-01-05T08:00:00", exceptions, task);
    let mut imported = parse_mspdi(&xml).expect("working exception project should import");
    let id = task_id(&imported, 1);
    let ef = imported
        .schedule
        .compute()
        .expect("working exception project should compute")
        .get(id)
        .ef;
    let applied = imported
        .schedule
        .calendars()
        .first()
        .expect("project should import its calendar")
        .exceptions()
        .to_vec();
    (ef, imported.unsupported, applied)
}

#[test]
fn a_working_saturday_moves_a_task_finish() {
    let (without, _, _) = finish_of_six_working_days("");
    // Six 8-hour days: Monday to Friday, then the next Monday at 16:00.
    assert_eq!(without, 7 * 1440 + 16 * 60);

    let saturday = working_exception(
        "2026-01-10",
        "2026-01-10",
        "<WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>",
    );
    let (with, unsupported, applied) = finish_of_six_working_days(&saturday);
    // The sixth day is that Saturday (day 5), finishing at 16:00.
    assert_eq!(with, 5 * 1440 + 16 * 60);
    assert_eq!(applied, [(5, vec![(480, 960)])]);
    assert!(unsupported.is_empty(), "{unsupported:?}");
}

#[test]
fn a_working_exception_range_covers_every_date_and_repeats_are_deduplicated() {
    let times = "<WorkingTime><FromTime>09:00:00</FromTime><ToTime>11:00:00</ToTime></WorkingTime>";
    let range = working_exception("2026-01-10", "2026-01-11", times);
    // A second, different exception on the first date: dropped, the first stays.
    let repeat = working_exception(
        "2026-01-10",
        "2026-01-10",
        "<WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>",
    );
    let (_, unsupported, applied) = finish_of_six_working_days(&format!("{range}{repeat}"));

    assert_eq!(applied, [(5, vec![(540, 660)]), (6, vec![(540, 660)])]);
    assert_eq!(
        unsupported,
        [
            "Calendar 1: exceptions that repeat a date with different hours were not read (1); the first on each date is used, so some days can differ from the file's."
        ]
    );
}

#[test]
fn a_working_exception_without_times_is_noted_not_turned_into_a_holiday() {
    let bare = "<Exception><TimePeriod><FromDate>2026-01-07T00:00:00</FromDate>\
<ToDate>2026-01-07T23:59:00</ToDate></TimePeriod><DayWorking>1</DayWorking></Exception>";
    let (_, unsupported, applied) = finish_of_six_working_days(bare);

    assert_eq!(applied, Vec::<(i64, Vec<(u16, u16)>)>::new());
    assert_eq!(
        unsupported,
        [
            "Calendar 1: a working exception with no working times was not read; those days keep the calendar's usual week."
        ]
    );
}

const TIMES: &str =
    "<WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime>";

/// A recurring exception: `pattern` holds the Type and its fields, the periods replace the
/// weekly pattern on every occurrence (`working`) or the day is a holiday.
fn recurring(from: &str, to: &str, pattern: &str, working: bool) -> String {
    let times = if working {
        format!("<WorkingTimes>{TIMES}</WorkingTimes>")
    } else {
        String::new()
    };
    format!(
        "<Exception><EnteredByOccurrences>0</EnteredByOccurrences><TimePeriod><FromDate>{from}T00:00:00</FromDate>\
<ToDate>{to}T23:59:00</ToDate></TimePeriod>{pattern}<DayWorking>{}</DayWorking>{times}</Exception>",
        u8::from(working)
    )
}

/// A work week from `from` to `to` working Monday to Saturday (`days` is the `DayType` list).
fn work_week(from: &str, to: &str, days: &[u8]) -> String {
    let week_days: String = (1..=7u8)
        .map(|day| {
            if days.contains(&day) {
                format!("<WeekDay><DayType>{day}</DayType><DayWorking>1</DayWorking><WorkingTimes>{TIMES}</WorkingTimes></WeekDay>")
            } else {
                format!("<WeekDay><DayType>{day}</DayType><DayWorking>0</DayWorking></WeekDay>")
            }
        })
        .collect();
    format!(
        "<WorkWeek><TimePeriod><FromDate>{from}T00:00:00</FromDate><ToDate>{to}T23:59:00</ToDate></TimePeriod>\
<Name>Saturdays</Name><WeekDays>{week_days}</WeekDays></WorkWeek>"
    )
}

/// The finish minute (from the project start day) of one task of `hours` 8-hour-day hours, and
/// the import notes, for a Mon-Fri calendar with the given exceptions and work weeks.
fn finish_of(start: &str, hours: u32, exceptions: &str, work_weeks: &str) -> (i64, Vec<String>) {
    let task = format!(
        "<Task><UID>1</UID><Name>T</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT{hours}H0M0S</Duration><DurationFormat>7</DurationFormat></Task>"
    );
    let xml = project(start, exceptions, &task).replace(
        "</Exceptions>",
        &format!("</Exceptions><WorkWeeks>{work_weeks}</WorkWeeks>"),
    );
    let mut imported = parse_mspdi(&xml).expect("calendar project should import");
    let id = task_id(&imported, 1);
    let ef = imported
        .schedule
        .compute()
        .expect("calendar project should compute")
        .get(id)
        .ef;
    (ef, imported.unsupported)
}

const DAY: i64 = 1440;
const FOUR: i64 = 16 * 60;

#[test]
fn a_work_week_with_a_working_saturday_moves_a_task_finish() {
    // Mon-Sat from 2026-01-01: six 8-hour days from Monday 2026-01-05 end on Saturday 01-10.
    let week = work_week("2026-01-01", "2026-01-31", &[2, 3, 4, 5, 6, 7]);
    let (finish, notes) = finish_of("2026-01-05T08:00:00", 48, "", &week);
    assert_eq!(finish, 5 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");

    // The same week ending on Friday 01-09 leaves that Saturday off: the sixth day is Monday 01-12.
    let short = work_week("2026-01-01", "2026-01-09", &[2, 3, 4, 5, 6, 7]);
    let (finish, _) = finish_of("2026-01-05T08:00:00", 48, "", &short);
    assert_eq!(finish, 7 * DAY + FOUR);
}

/// A work week that lists only the given `(DayType, DayWorking)` weekdays, as MS Project writes
/// a week whose other days use the project default times.
fn partial_work_week(from: &str, to: &str, listed: &[(u8, u8)]) -> String {
    let week_days: String = listed
        .iter()
        .map(|(day, working)| {
            let times = if *working == 1 {
                format!("<WorkingTimes>{TIMES}</WorkingTimes>")
            } else {
                String::new()
            };
            format!("<WeekDay><DayType>{day}</DayType><DayWorking>{working}</DayWorking>{times}</WeekDay>")
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "<WorkWeek><TimePeriod><FromDate>{from}T00:00:00</FromDate><ToDate>{to}T23:59:00</ToDate></TimePeriod>\
<Name>Saturdays</Name><WeekDays>{week_days}</WeekDays></WorkWeek>"
    )
}

#[test]
fn a_work_week_that_lists_only_saturday_leaves_the_other_days_on_the_calendar_week() {
    // Only Saturday (DayType 7) is listed; Monday to Friday keep the calendar's week, so six
    // 8-hour days from Monday 2026-01-05 end on Saturday 01-10.
    let week = partial_work_week("2026-01-01", "2026-01-31", &[(7, 1)]);
    let (finish, notes) = finish_of("2026-01-05T08:00:00", 48, "", &week);
    assert_eq!(finish, 5 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_listed_weekday_that_is_not_worked_stays_off_in_a_work_week() {
    // Saturday works and Wednesday is listed as not worked: Mon 05, Tue 06, Thu 08, Fri 09,
    // Sat 10, then Monday 01-12.
    let week = partial_work_week("2026-01-01", "2026-01-31", &[(7, 1), (4, 0)]);
    let (finish, _) = finish_of("2026-01-05T08:00:00", 48, "", &week);
    assert_eq!(finish, 7 * DAY + FOUR);
}

#[test]
fn an_exception_beats_the_work_week_on_its_date() {
    let week = work_week("2026-01-01", "2026-01-31", &[2, 3, 4, 5, 6, 7]);
    // Wednesday 01-07 is a holiday: Mon, Tue, Thu, Fri, Sat, then Monday 01-12.
    let holiday = "<Exception><TimePeriod><FromDate>2026-01-07T00:00:00</FromDate>\
<ToDate>2026-01-07T23:59:00</ToDate></TimePeriod><DayWorking>0</DayWorking></Exception>";
    let (finish, _) = finish_of("2026-01-05T08:00:00", 48, holiday, &week);
    assert_eq!(finish, 7 * DAY + FOUR);
}

#[test]
fn a_weekly_recurring_working_saturday_moves_a_task_finish() {
    // Every Saturday 01-10 to 01-31 works (DaysOfWeek 64 is Saturday). Twelve days:
    // Mon-Fri, Sat 01-10, Mon-Fri, Sat 01-17.
    let saturdays = recurring(
        "2026-01-10",
        "2026-01-31",
        "<Type>6</Type><Period>1</Period><DaysOfWeek>64</DaysOfWeek>",
        true,
    );
    let (finish, notes) = finish_of("2026-01-05T08:00:00", 96, &saturdays, "");
    assert_eq!(finish, 12 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");
    // Without it: Mon-Fri, Mon-Fri, Mon 01-19, Tue 01-20.
    assert_eq!(
        finish_of("2026-01-05T08:00:00", 96, "", "").0,
        15 * DAY + FOUR
    );
}

#[test]
fn a_daily_recurring_holiday_every_second_day_moves_a_task_finish() {
    // Off on 01-06, 08, 10, 12 and 14: four days are Mon 05, Wed 07, Fri 09, Tue 13.
    let every_other = recurring(
        "2026-01-06",
        "2026-01-14",
        "<Type>7</Type><Period>2</Period>",
        false,
    );
    let (finish, notes) = finish_of("2026-01-05T08:00:00", 32, &every_other, "");
    assert_eq!(finish, 8 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_monthly_recurring_holiday_applies_on_each_month() {
    // The 4th of each month from 2026-01-04 to 2026-03-04. The project starts Monday 02-02, so
    // the January date is before it; Wed 02-04 is off: Mon 02, Tue 03, Thu 05.
    let fourth = recurring(
        "2026-01-04",
        "2026-03-04",
        "<Type>4</Type><Period>1</Period><MonthDay>4</MonthDay>",
        false,
    );
    let (finish, notes) = finish_of("2026-02-02T08:00:00", 24, &fourth, "");
    assert_eq!(finish, 3 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_yearly_recurring_holiday_applies_in_a_later_year() {
    // December 23 each year (Month 11 is December) from 2026 to 2028. The project starts
    // Monday 2027-12-20: Mon 20, Tue 21, Wed 22, then Fri 24 because Thu 23 is off.
    let christmas_eve_eve = recurring(
        "2026-12-23",
        "2028-12-23",
        "<Type>2</Type><Month>11</Month><MonthDay>23</MonthDay>",
        false,
    );
    let (finish, notes) = finish_of("2027-12-20T08:00:00", 32, &christmas_eve_eve, "");
    assert_eq!(finish, 4 * DAY + FOUR);
    assert!(notes.is_empty(), "{notes:?}");
}
