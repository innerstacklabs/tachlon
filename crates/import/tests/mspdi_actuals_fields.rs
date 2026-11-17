//! gs#703: the MS Project reader leaves a task's `actual_start`, `actual_finish` and `remaining`
//! empty when the file's actuals cannot be used: a finish with no start, an unreadable start, an
//! unreadable finish, and a split task's `Resume`. A finished task with a stated remaining
//! duration of zero keeps all three. Only the task fields are pinned here, not the notes.

use engine::Instant;
use import::parse_mspdi;

const DAY: Instant = 1_440;
const HOUR: Instant = 60;

const WORKING: &str = "<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime>\
<ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>";

/// A Monday-to-Friday, 08:00 to 16:00 project starting Monday 2026-01-05 (instant 0 is that
/// Monday at 00:00).
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

fn task(uid: u32, extra: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel>\
<Summary>0</Summary><Duration>PT24H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
    )
}

#[test]
fn dropped_actuals_leave_the_task_fields_empty() {
    let tasks = [
        task(1, "<ActualFinish>2026-01-07T16:00:00</ActualFinish>"),
        task(2, "<ActualStart>soon</ActualStart>"),
        task(
            3,
            "<ActualStart>2026-01-06T08:00:00</ActualStart><ActualFinish>later</ActualFinish>",
        ),
        task(
            4,
            "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<ActualFinish>2026-01-07T16:00:00</ActualFinish>\
<RemainingDuration>PT0H0M0S</RemainingDuration>",
        ),
        task(
            5,
            "<ActualStart>2026-01-06T08:00:00</ActualStart>\
<Resume>2026-01-07T08:00:00</Resume>",
        ),
    ]
    .concat();
    let imported = parse_mspdi(&project(&tasks)).expect("import");
    assert_eq!(imported.tasks.len(), 5);

    let start = Some(DAY + 8 * HOUR);
    let finish = Some(2 * DAY + 16 * HOUR);
    let fields: Vec<_> = imported
        .tasks
        .iter()
        .map(|task| (task.actual_start, task.actual_finish, task.remaining))
        .collect();
    assert_eq!(
        fields,
        vec![
            (None, None, None),
            (None, None, None),
            (start, None, None),
            (start, finish, Some(0)),
            (start, None, None),
        ]
    );
}
