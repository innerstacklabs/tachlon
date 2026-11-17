//! The MSPDI `Baseline` numbered 0 gives each task's baseline start and finish (gs#703).
//!
//! `fixtures/baseline.xml` is three tasks lifted verbatim from MPXJ's Project 2007 test file
//! `task-baselines-project2007-mspdi.xml`: baseline 0 with only a `Finish`, baseline 0 with
//! only a `Start`, and a task that has only baseline 1. The tests below it build the other
//! shapes of that file inline.

use import::{Imported, parse_mspdi};

const BASELINE: &str = include_str!("fixtures/baseline.xml");

/// The plan starts on Monday 2015-02-09, so instant 0 is that Monday at 00:00.
const DAY: i64 = 1440;

fn imported_task(imported: &Imported, uid: u32) -> (Option<i64>, Option<i64>) {
    let task = imported
        .tasks
        .iter()
        .find(|task| task.uid == uid)
        .expect("fixture task UID should be imported");
    (task.baseline_start, task.baseline_finish)
}

#[test]
fn a_baseline_0_that_states_only_a_finish_gives_only_a_baseline_finish() {
    let imported = parse_mspdi(BASELINE).expect("baseline fixture should import");
    // 2014-03-01T09:00 is 345 days before the origin.
    assert_eq!(
        imported_task(&imported, 23),
        (None, Some(-345 * DAY + 9 * 60))
    );
}

#[test]
fn a_baseline_0_that_states_only_a_start_gives_only_a_baseline_start() {
    let imported = parse_mspdi(BASELINE).expect("baseline fixture should import");
    // 2014-04-01T09:00 is 314 days before the origin.
    assert_eq!(
        imported_task(&imported, 34),
        (Some(-314 * DAY + 9 * 60), None)
    );
}

#[test]
fn only_baseline_0_is_read() {
    let imported = parse_mspdi(BASELINE).expect("baseline fixture should import");
    // Task 35 states a start in baseline 1 and nothing in baseline 0.
    assert_eq!(imported_task(&imported, 35), (None, None));
}

const WEEK: &str = "<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>\
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>\
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>";

/// A project starting on Monday 2026-01-05 with one task whose body is `extra`.
fn one_task(extra: &str) -> Imported {
    let xml = format!(
        "<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-05T08:00:00</StartDate><CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID>\
<Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>\
<WeekDays>{WEEK}</WeekDays></Calendar></Calendars><Tasks><Task><UID>1</UID><Name>T</Name>\
<OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT16H0M0S</Duration>\
<DurationFormat>7</DurationFormat>{extra}</Task></Tasks></Project>"
    );
    parse_mspdi(&xml).expect("the project should import")
}

#[test]
fn a_baseline_0_that_states_both_dates_gives_both_after_the_origin() {
    // The element shape of MPXJ's `baselines.xml` (Start, Finish, Duration, Work, Cost), numbered 0.
    let imported = one_task(
        "<Baseline><Number>0</Number><Start>2026-01-06T09:00:00</Start>\
<Finish>2026-01-07T17:30:00</Finish><Duration>PT7H30M0S</Duration></Baseline>",
    );
    assert_eq!(
        imported_task(&imported, 1),
        (Some(DAY + 9 * 60), Some(2 * DAY + 17 * 60 + 30))
    );
}

#[test]
fn a_baseline_0_that_states_only_a_cost_gives_neither() {
    // The shape of the sample's cost baselines: `Number` and `Cost`, no dates. The task states
    // its own dates, which a baseline must never borrow.
    let own_dates = "<Start>2026-01-05T08:00:00</Start><Finish>2026-01-06T16:00:00</Finish>";
    let imported = one_task(&format!(
        "{own_dates}<Baseline><Number>0</Number><Cost>100</Cost></Baseline>"
    ));
    assert_eq!(imported_task(&imported, 1), (None, None));
    // The same task with a baseline start does read it, so the `None` above is the baseline's.
    let imported = one_task(&format!(
        "{own_dates}<Baseline><Number>0</Number><Start>2026-01-06T09:00:00</Start></Baseline>"
    ));
    assert_eq!(imported_task(&imported, 1), (Some(DAY + 9 * 60), None));
}

#[test]
fn an_unreadable_baseline_date_is_dropped_and_its_sibling_stays() {
    let imported = one_task(
        "<Baseline><Number>0</Number><Start>not a date</Start>\
<Finish>2026-01-07T17:30:00</Finish></Baseline>",
    );
    assert_eq!(
        imported_task(&imported, 1),
        (None, Some(2 * DAY + 17 * 60 + 30))
    );
}

#[test]
fn a_baseline_0_after_other_baselines_is_found() {
    let imported = one_task(
        "<Baseline><Number>1</Number><Start>2026-01-05T08:00:00</Start></Baseline>\
<Baseline><Number>0</Number><Start>2026-01-06T09:00:00</Start></Baseline>",
    );
    assert_eq!(imported_task(&imported, 1), (Some(DAY + 9 * 60), None));
}
