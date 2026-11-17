//! The MSPDI `Start`, `Finish` and `ActualStart` feed the panel's file-dates warning (gs#691).

use import::{Imported, parse_mspdi};

/// Starts on Monday 2026-01-05 08:00, so instant 0 is that Monday at 00:00.
const DAY: i64 = 1440;

const WEEK: &str = "<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>";

fn import(tasks: &str) -> Imported {
    let xml = format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-05T08:00:00</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID>\
<Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{WEEK}</WeekDays></Calendar></Calendars><Tasks>{tasks}</Tasks></Project>"
    );
    parse_mspdi(&xml).expect("the project should import")
}

fn task(uid: u32, extra: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT16H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
    )
}

const DATES: &str = "<Start>2026-01-05T08:00:00</Start><Finish>2026-01-06T16:00:00</Finish>";

#[test]
fn a_started_task_is_in_progress_and_keeps_the_files_dates_from_the_origin() {
    let imported = import(&task(
        1,
        &format!("{DATES}<ActualStart>2026-01-05T08:00:00</ActualStart>"),
    ));
    let started = &imported.tasks[0];
    assert!(started.in_progress);
    assert_eq!(started.file_start, Some(8 * 60));
    assert_eq!(started.file_finish, Some(DAY + 16 * 60));
}

#[test]
fn a_task_with_an_actual_finish_is_not_in_progress() {
    let actuals = "<ActualStart>2026-01-05T08:00:00</ActualStart><ActualFinish>2026-01-06T16:00:00</ActualFinish>";
    let imported = import(&task(1, &format!("{DATES}{actuals}")));
    assert!(!imported.tasks[0].in_progress);
}

#[test]
fn a_task_with_no_actual_start_is_not_in_progress() {
    let imported = import(&task(1, DATES));
    let waiting = &imported.tasks[0];
    assert!(!waiting.in_progress);
    assert_eq!(waiting.file_start, Some(8 * 60));
}

#[test]
fn an_empty_or_unreadable_date_is_none_and_does_not_fail_the_import() {
    let empty = import(&task(
        1,
        "<Start></Start><Finish></Finish><ActualStart></ActualStart>",
    ));
    assert_eq!(empty.tasks[0].file_start, None);
    assert_eq!(empty.tasks[0].file_finish, None);
    assert!(!empty.tasks[0].in_progress);
    let bad = import(&task(1, "<Start>soon</Start><Finish>later</Finish>"));
    assert_eq!(bad.tasks[0].file_start, None);
    assert_eq!(bad.tasks[0].file_finish, None);
}

#[test]
fn a_summary_has_no_file_dates_and_is_never_in_progress() {
    let summary = format!(
        "<Task><UID>1</UID><Name>S</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>\
{DATES}<ActualStart>2026-01-05T08:00:00</ActualStart></Task>"
    );
    let imported = import(&summary);
    let task = &imported.tasks[0];
    assert!(!task.in_progress);
    assert_eq!((task.file_start, task.file_finish), (None, None));
}
