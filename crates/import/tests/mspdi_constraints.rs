//! gantt#1875: an MS Project task's constraint types reach the engine as limits.

use std::fmt::Write as _;

use engine::{Limit, Pin};
use import::{Imported, parse_mspdi};

const CONSTRAINTS: &str = include_str!("fixtures/constraints.xml");

// Instants from the origin Monday 2026-01-05; the calendar works Monday to Friday, 08:00 to 16:00.
const THU_0800: i64 = 3 * 1_440 + 480;
const THU_1600: i64 = 3 * 1_440 + 960;
const FRI_0800: i64 = 4 * 1_440 + 480;

/// A project of the given tasks, each (type, date, hours): a type 0 has no constraint.
fn import(tasks: &[(u32, &str, u32)]) -> Imported {
    let day = "<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes>";
    let mut xml = format!(
        "<Project><StartDate>2026-01-05T08:00:00</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID><WeekDays><WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking>{day}</WeekDay><WeekDay><DayType>3</DayType><DayWorking>1</DayWorking>{day}</WeekDay><WeekDay><DayType>4</DayType><DayWorking>1</DayWorking>{day}</WeekDay><WeekDay><DayType>5</DayType><DayWorking>1</DayWorking>{day}</WeekDay><WeekDay><DayType>6</DayType><DayWorking>1</DayWorking>{day}</WeekDay><WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay></WeekDays></Calendar></Calendars><Tasks>"
    );
    for (uid, (kind, date, hours)) in (1..).zip(tasks) {
        let date = if date.is_empty() {
            String::new()
        } else {
            format!("<ConstraintDate>{date}</ConstraintDate>")
        };
        write!(
            xml,
            "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Duration>PT{hours}H0M0S</Duration><DurationFormat>7</DurationFormat><ConstraintType>{kind}</ConstraintType>{date}</Task>"
        )
        .expect("a String takes writes");
    }
    xml.push_str("</Tasks></Project>");
    parse_mspdi(&xml).expect("constraints import")
}

fn with(set: impl Fn(&mut Limit)) -> Limit {
    let mut limit = Limit::default();
    set(&mut limit);
    limit
}

#[test]
fn a_must_start_on_task_starts_on_its_date() {
    let mut imported = parse_mspdi(CONSTRAINTS).expect("the fixture imports");
    let solution = imported.schedule.compute().expect("the fixture computes");
    // Task 2 is the second task, must start on Tuesday 2026-01-06 08:00; the project starts Monday.
    assert_eq!(solution.get(engine::TaskId(1)).es, 1_440 + 480);
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|line| line.contains("Task 2:")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn every_constraint_type_maps_to_its_limit() {
    let cases: [(u32, &str, Limit); 6] = [
        (1, "", with(|limit| limit.alap = true)),
        (
            2,
            "2026-01-08T08:00:00",
            with(|l| l.pin = Some(Pin::Start(THU_0800))),
        ),
        (
            3,
            "2026-01-08T16:00:00",
            with(|l| l.pin = Some(Pin::Finish(THU_1600))),
        ),
        (5, "2026-01-08T08:00:00", with(|l| l.snlt = Some(THU_0800))),
        (6, "2026-01-08T16:00:00", with(|l| l.fnet = Some(THU_1600))),
        (7, "2026-01-08T16:00:00", with(|l| l.fnlt = Some(THU_1600))),
    ];
    for (kind, date, expected) in cases {
        let imported = import(&[(kind, date, 8)]);
        let task = imported.schedule.tasks().first().expect("one task");
        assert_eq!(task.limit, expected, "type {kind}");
        assert_eq!(imported.unsupported, Vec::<String>::new(), "type {kind}");
    }
}

#[test]
fn a_must_finish_on_task_finishes_on_its_date() {
    let mut imported = import(&[(3, "2026-01-08T16:00:00", 8)]);
    let solution = imported.schedule.compute().expect("computes");
    let task = solution.get(engine::TaskId(0));
    assert_eq!((task.es, task.ef), (THU_0800, THU_1600));
}

#[test]
fn an_as_late_as_possible_task_starts_as_late_as_the_project_finish_allows() {
    // The 40-hour task runs the whole week, so the 8-hour one starts on its last day.
    let mut imported = import(&[(0, "", 40), (1, "", 8)]);
    let solution = imported.schedule.compute().expect("computes");
    assert_eq!(solution.get(engine::TaskId(1)).es, FRI_0800);
}

#[test]
fn a_type_with_no_date_stays_a_note() {
    let imported = import(&[(5, "", 8)]);
    assert_eq!(
        imported.unsupported,
        [
            "Task 1: the start no later than constraint (type 5) has no date and was not read; the task is scheduled without it, so its dates can differ from the file's."
        ]
    );
    assert_eq!(
        imported.schedule.tasks().first().expect("one task").limit,
        Limit::default()
    );
}

#[test]
fn a_summary_takes_no_limit() {
    let summary = CONSTRAINTS.replacen(
        "<Summary>0</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>\n      <ConstraintType>2",
        "<Summary>1</Summary><Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>\n      <ConstraintType>2",
        1,
    );
    assert_ne!(summary, CONSTRAINTS);
    let imported = parse_mspdi(&summary).expect("the patched fixture imports");
    let task = imported.schedule.tasks().get(1).expect("task 2");
    assert_eq!(task.limit, Limit::default());
}
