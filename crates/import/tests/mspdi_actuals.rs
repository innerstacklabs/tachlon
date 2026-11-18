//! gs#860: an MS Project task's `ActualStart`, `ActualFinish` and `RemainingDuration` reach the
//! imported task, and the engine schedules them as decision 0008 says, as for an XER activity
//! (gs#703).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Instant, Solution};
use import::{Imported, parse_mspdi};

const DAY: Instant = 1_440;
const HOUR: Instant = 60;

const WORKING: &str = "<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime>\
<ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>";

/// Monday to Friday, 08:00 to 16:00. The project starts Monday 2026-01-05 08:00 (instant 0 is
/// that Monday at 00:00) with its status date on Thursday 2026-01-08 08:00.
fn project(tasks: &str) -> String {
    let weekdays: String = (2..=6)
        .map(|day| {
            format!(
                "<WeekDay><DayType>{day}</DayType><DayWorking>1</DayWorking>{WORKING}</WeekDay>"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-05T08:00:00</StartDate><StatusDate>2026-01-08T08:00:00</StatusDate>\
<CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID><Name>Standard</Name>\
<IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays><WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>{weekdays}\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay></WeekDays></Calendar>\
</Calendars><Tasks>{tasks}</Tasks></Project>"
    )
}

fn task(uid: u32, summary: bool, extra: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel>\
<Summary>{}</Summary><Duration>PT24H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>",
        u8::from(summary)
    )
}

/// The schedule as the app builds it: the data date and each task's actuals handed to the engine
/// before the first compute.
fn solve(imported: &mut Imported) -> Solution {
    imported
        .schedule
        .set_data_date(imported.data_date)
        .expect("in range");
    for (index, task) in imported.tasks.iter().enumerate() {
        let id = engine::TaskId(index as u32);
        imported
            .schedule
            .set_actual_start(id, task.actual_start)
            .expect("live task");
        imported
            .schedule
            .set_actual_finish(id, task.actual_finish)
            .expect("live task");
        imported
            .schedule
            .set_remaining(id, task.remaining)
            .expect("live task");
    }
    imported.schedule.compute().expect("compute")
}

#[test]
fn an_in_progress_task_keeps_its_actual_start_and_remaining() {
    // Three days of work (24 h), started Tuesday 08:00, one day (8 h) remaining.
    let xml = project(&task(
        1,
        false,
        "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<RemainingDuration>PT8H0M0S</RemainingDuration>",
    ));
    let mut imported = parse_mspdi(&xml).expect("import");
    let task = &imported.tasks[0];
    assert_eq!(task.actual_start, Some(DAY + 8 * HOUR));
    assert_eq!(task.actual_finish, None);
    assert_eq!(task.remaining, Some(8 * HOUR));
    assert_eq!(imported.data_date, Some(3 * DAY + 8 * HOUR));
    assert_eq!(imported.unsupported, Vec::<String>::new());

    // The start stays at the actual start. The one remaining day cannot start before the data
    // date, Thursday 08:00, so it ends Thursday 16:00.
    let solution = solve(&mut imported);
    assert_eq!(solution.dates[0].es, DAY + 8 * HOUR);
    assert_eq!(solution.dates[0].ef, 3 * DAY + 16 * HOUR);
}

#[test]
fn a_finished_task_is_fixed_at_its_actual_dates() {
    // Planned 24 h, finished Monday 08:00 to Tuesday 16:00, before the status date.
    let xml = project(&task(
        1,
        false,
        "<ActualStart>2026-01-05T08:00:00</ActualStart>\
<ActualFinish>2026-01-06T16:00:00</ActualFinish><RemainingDuration>PT0H0M0S</RemainingDuration>",
    ));
    let mut imported = parse_mspdi(&xml).expect("import");
    let task = &imported.tasks[0];
    assert_eq!(task.actual_start, Some(8 * HOUR));
    assert_eq!(task.actual_finish, Some(DAY + 16 * HOUR));
    assert!(!task.in_progress);

    let solution = solve(&mut imported);
    assert_eq!(solution.dates[0].es, 8 * HOUR);
    assert_eq!(solution.dates[0].ef, DAY + 16 * HOUR);
}

#[test]
fn a_summary_takes_none_while_its_child_keeps_its_actuals() {
    let started = "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<RemainingDuration>PT8H0M0S</RemainingDuration>";
    let xml = project(&(task(1, true, started) + &task(2, false, started)));
    let imported = parse_mspdi(&xml).expect("import");
    let summary = &imported.tasks[0];
    assert_eq!(summary.actual_start, None);
    assert_eq!(summary.actual_finish, None);
    assert_eq!(summary.remaining, None);
    assert_eq!(imported.tasks[1].actual_start, Some(DAY + 8 * HOUR));
    assert_eq!(imported.tasks[1].remaining, Some(8 * HOUR));
}

#[test]
fn a_remaining_duration_or_percent_alone_moves_no_date() {
    let started = "<ActualStart>2026-01-06T08:00:00</ActualStart>";
    let xml = project(
        &(task(1, false, started)
            + &task(2, false, "<RemainingDuration>PT8H0M0S</RemainingDuration>")
            + &task(3, false, "<PercentComplete>50</PercentComplete>")),
    );
    let imported = parse_mspdi(&xml).expect("import");
    assert_eq!(imported.tasks[0].actual_start, Some(DAY + 8 * HOUR));
    // A remaining duration needs a start to hang from; a percent moves no date.
    assert_eq!(imported.tasks[1].remaining, None);
    assert_eq!(imported.tasks[2].actual_start, None);
    assert_eq!(imported.tasks[2].remaining, None);
    assert_eq!(imported.tasks[2].progress, Some(50));
}

#[test]
fn every_actual_the_reader_drops_is_listed() {
    let xml = project(
        &(task(
            1,
            false,
            "<ActualFinish>2026-01-06T16:00:00</ActualFinish>",
        ) + &task(2, false, "<ActualStart>soon</ActualStart>")
            + &task(
                3,
                false,
                "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<Resume>2026-01-07T08:00:00</Resume><Stop>2026-01-06T12:00:00</Stop>",
            )
            + &task(
                4,
                false,
                "<ActualStart>2026-01-06T08:00:00</ActualStart><ActualFinish>later</ActualFinish>",
            )
            // A summary is rolled up by the app: no line, whatever it carries.
            + &task(
                5,
                true,
                "<ActualStart>2026-01-06T08:00:00</ActualStart><Resume>2026-01-07T08:00:00</Resume>",
            )),
    );
    let imported = parse_mspdi(&xml).expect("import");
    assert_eq!(
        imported.unsupported,
        [
            "Task 1: the actual finish was not read because the task has no actual start; its dates can differ from the file's.",
            "Task 2: the actual start \"soon\" is not a date, so its actual dates were not read; the task is scheduled as not started and its dates can differ from the file's.",
            "Task 3: the stop date was not read; no dates are affected.",
            "Task 4: the actual finish \"later\" is not a date and was not read; the task is not fixed at a finish and its dates can differ from the file's.",
        ]
    );
}

#[test]
fn a_started_task_keeps_the_date_its_remaining_work_resumes() {
    let started = "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<RemainingDuration>PT8H0M0S</RemainingDuration>";
    let split = format!("{started}<Resume>2026-01-12T08:00:00</Resume>");
    let mut with = parse_mspdi(&project(&task(1, false, &split))).expect("import");
    let mut without = parse_mspdi(&project(&task(1, false, started))).expect("import");
    assert_eq!(
        with.schedule.tasks()[0].limit.resume,
        Some(7 * DAY + 8 * HOUR)
    );
    assert_eq!(without.schedule.tasks()[0].limit.resume, None);
    assert_eq!(with.unsupported, Vec::<String>::new());

    // The one remaining day waits for the resume date, Monday 08:00 a week later, not Thursday.
    assert_eq!(solve(&mut without).dates[0].ef, 3 * DAY + 16 * HOUR);
    assert_eq!(solve(&mut with).dates[0].ef, 7 * DAY + 16 * HOUR);
}
