//! Elapsed MS Project durations (gs#690): a task whose `DurationFormat` is an elapsed unit
//! (`em`, `eh`, `ed`, `ew`, `emo`) counts its `Duration` in all minutes, so it spans weekends.
//!
//! Recorded premise: MS Project writes 7 elapsed days as `<Duration>PT168H0M0S</Duration>` with
//! `<DurationFormat>8</DurationFormat>`: elapsed days are 24 hours each, never the working day
//! of the calendar. No MS Project-written sample is in the repository, so the tests build the
//! element from that rule.

use engine::TaskId;
use import::{Imported, parse_mspdi};

const WEEK: &str = "<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>";

/// A Mon-Fri 08:00 to 16:00 project, calendar 1, starting at `start`.
fn project(start: &str, tasks: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>{start}</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID>\
<Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{WEEK}</WeekDays></Calendar></Calendars><Tasks>{tasks}</Tasks></Project>"
    )
}

fn task(uid: u32, duration: &str, format: u32, extra: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>{duration}</Duration><DurationFormat>{format}</DurationFormat>{extra}</Task>"
    )
}

fn id_of(imported: &Imported, uid: u32) -> TaskId {
    let position = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid)
        .expect("fixture task UID should be imported");
    TaskId(u32::try_from(position).expect("fixture task count should fit in u32"))
}

/// The task's start and finish as instants from the origin Monday 00:00.
fn dates(imported: &mut Imported, uid: u32) -> (i64, i64) {
    let id = id_of(imported, uid);
    let solution = imported
        .schedule
        .compute()
        .expect("fixture schedule should compute");
    let found = solution.get(id);
    (found.es, found.ef)
}

// Monday 2026-01-05 is the origin: 08:00 that day is instant 480.
const MONDAY_START: &str = "2026-01-05T08:00:00";
const MONDAY_START_INSTANT: i64 = 480;
// Friday 2026-01-09 08:00 is instant 4 days and 8 hours after the origin.
const FRIDAY_START: &str = "2026-01-09T08:00:00";
const FRIDAY_START_INSTANT: i64 = 4 * 1_440 + 480;

#[test]
fn seven_elapsed_days_span_weekends() {
    let tasks = task(1, "PT168H0M0S", 8, "");
    let mut imported = parse_mspdi(&project(MONDAY_START, &tasks)).expect("file should parse");

    let (start, finish) = dates(&mut imported, 1);

    assert_eq!(start, MONDAY_START_INSTANT);
    // Seven 24-hour days from Monday 08:00 end the next Monday 08:00, weekend included.
    assert_eq!(finish, MONDAY_START_INSTANT + 7 * 1_440);
    assert!(
        imported.unsupported.is_empty(),
        "an elapsed duration is supported now: {:?}",
        imported.unsupported
    );
}

#[test]
fn seven_working_days_still_skip_weekends() {
    // The same seven days in working time (8-hour days, format 7) end Tuesday 16:00 of week two.
    let tasks = task(1, "PT56H0M0S", 7, "");
    let mut imported = parse_mspdi(&project(MONDAY_START, &tasks)).expect("file should parse");

    let (start, finish) = dates(&mut imported, 1);

    assert_eq!(start, MONDAY_START_INSTANT);
    assert_eq!(finish, 8 * 1_440 + 16 * 60);
}

#[test]
fn every_elapsed_unit_counts_all_minutes() {
    // em, eh, ed, ew, emo and the estimated edays (40 is 8 plus the estimate flag, 32).
    let formats = [4, 6, 8, 10, 12, 40];
    let tasks: String = formats
        .iter()
        .enumerate()
        .map(|(index, format)| {
            let uid = u32::try_from(index).expect("small index") + 1;
            task(uid, "PT24H0M0S", *format, "")
        })
        .collect();
    let mut imported = parse_mspdi(&project(FRIDAY_START, &tasks)).expect("file should parse");

    for (index, format) in formats.iter().enumerate() {
        let uid = u32::try_from(index).expect("small index") + 1;
        let (start, finish) = dates(&mut imported, uid);
        assert_eq!(start, FRIDAY_START_INSTANT, "format {format}");
        // 24 elapsed hours from Friday 08:00 end Saturday 08:00.
        assert_eq!(finish, FRIDAY_START_INSTANT + 1_440, "format {format}");
    }
}

#[test]
fn an_elapsed_task_ignores_its_own_calendar() {
    let tasks = task(1, "PT24H0M0S", 8, "<CalendarUID>1</CalendarUID>");
    let mut imported = parse_mspdi(&project(FRIDAY_START, &tasks)).expect("file should parse");

    let (_, finish) = dates(&mut imported, 1);

    assert_eq!(finish, FRIDAY_START_INSTANT + 1_440);
}

#[test]
fn a_working_task_keeps_the_project_calendar_and_adds_no_calendar() {
    let tasks = task(1, "PT8H0M0S", 7, "");
    let imported = parse_mspdi(&project(MONDAY_START, &tasks)).expect("file should parse");

    assert_eq!(imported.schedule.calendars().len(), 1);
    let id = id_of(&imported, 1);
    assert_eq!(imported.schedule.tasks()[id.0 as usize].calendar.0, 0);
}

#[test]
fn an_elapsed_milestone_and_summary_keep_the_project_calendar() {
    let milestone = task(1, "PT0H0M0S", 8, "<Milestone>1</Milestone>");
    let summary = "<Task><UID>2</UID><Name>S</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>\
<Duration>PT24H0M0S</Duration><DurationFormat>8</DurationFormat></Task>";
    let imported = parse_mspdi(&project(MONDAY_START, &format!("{milestone}{summary}")))
        .expect("file should parse");

    for uid in [1, 2] {
        let id = id_of(&imported, uid);
        let task = &imported.schedule.tasks()[id.0 as usize];
        assert_eq!(task.duration, 0, "task {uid}");
        assert_eq!(task.calendar.0, 0, "task {uid}");
    }
    assert_eq!(imported.schedule.calendars().len(), 1);
}
