//! The four progress reads of the task table (gs#703): Actual start, Actual finish, Remaining and
//! % complete. The schedule is seven tasks of five working days (2400 minutes) on a Monday-to-Friday
//! calendar with 08:00-12:00 and 13:00-17:00 shifts, so a day is 480 minutes. Instant 0 is the demo
//! origin, Monday 2026-01-05 00:00, so Tuesday 09:30 is 1440 + 570 = 2010 and Wednesday 17:00 is
//! 2880 + 1020 = 3900.

use crate::common;

use app::{Session, meta::TaskMeta};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Minutes, Schedule, Task};
use timeline::Timeline;

const DURATION: Minutes = 2400;

fn task(
    calendar: engine::CalendarId,
    actual_start: Option<Instant>,
    actual_finish: Option<Instant>,
    remaining: Option<Minutes>,
) -> Task {
    Task {
        duration: DURATION,
        calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start,
        actual_finish,
        remaining,
    }
}

/// 0: started Tuesday 09:30. 1: started Monday 08:00, finished Wednesday 17:00. 2: only a
/// remaining of 600. 3: untouched. 4: started Monday 08:00, 600 remaining. 5: started Monday 08:00,
/// 1000 remaining. 6: started Monday 08:00, 900 remaining.
fn session(tasks: impl FnOnce(engine::CalendarId) -> Vec<Task>) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let tasks = tasks(calendar);
    let count = tasks.len();
    for t in tasks {
        schedule
            .add_task(t)
            .expect("the calendar belongs to the schedule");
    }
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new((0..count as u32).collect(), 1_000.0, 400.0, 1.0);
    Session::new(
        schedule,
        solution,
        timeline,
        vec![TaskMeta::default(); count],
        clock,
    )
}

fn progressed() -> Session {
    session(|c| {
        vec![
            task(c, Some(2010), None, None),
            task(c, Some(480), Some(3900), None),
            task(c, None, None, Some(600)),
            task(c, None, None, None),
            task(c, Some(480), None, Some(600)),
            task(c, Some(480), None, Some(1000)),
            task(c, Some(480), None, Some(900)),
        ]
    })
}

fn reads(s: &Session, i: u32) -> [String; 4] {
    [
        s.task_actual_start(i),
        s.task_actual_finish(i),
        s.task_remaining(i),
        s.task_percent(i),
    ]
}

#[test]
fn a_started_task_reads_its_actual_start_the_whole_duration_remaining_and_zero_percent() {
    assert_eq!(
        reads(&progressed(), 0),
        ["2026-01-06 09:30", "", "5d", "0%"]
    );
}

#[test]
fn a_finished_task_reads_both_actuals_nothing_remaining_and_one_hundred_percent() {
    assert_eq!(
        reads(&progressed(), 1),
        ["2026-01-05 08:00", "2026-01-07 17:00", "0", "100%"]
    );
}

#[test]
fn remaining_and_percent_come_from_the_remaining_duration() {
    // 600 minutes is 1 day and 120 minutes; (2400 - 600) / 2400 is 75%.
    assert_eq!(
        reads(&progressed(), 4),
        ["2026-01-05 08:00", "", "1d 2h", "75%"]
    );
    // 1000 minutes is 2 days (960) and 40 minutes; 1400 / 2400 is 58.33%, which reads 58%.
    assert_eq!(
        reads(&progressed(), 5),
        ["2026-01-05 08:00", "", "2d 40m", "58%"]
    );
}

#[test]
fn percent_rounds_half_up_to_a_whole_number() {
    // 900 minutes is 1 day and 420 minutes (7 hours); 1500 / 2400 is exactly 62.5%, which reads 63%.
    assert_eq!(
        reads(&progressed(), 6),
        ["2026-01-05 08:00", "", "1d 7h", "63%"]
    );
}

#[test]
fn work_not_started_reads_empty_even_with_a_remaining_the_engine_ignores() {
    let s = progressed();
    assert_eq!(reads(&s, 2), ["", "", "", ""]);
    assert_eq!(reads(&s, 3), ["", "", "", ""]);
}

#[test]
fn a_task_past_the_schedule_reads_empty() {
    assert_eq!(reads(&progressed(), 99), ["", "", "", ""]);
}

#[test]
fn only_a_schedule_with_progress_has_progress() {
    assert!(progressed().has_progress());
    let none = session(|c| vec![task(c, None, None, None)]);
    assert!(!none.has_progress());
    let percent_only = session(|c| vec![task(c, None, None, None)]).with_progress(vec![40]);
    assert!(
        !percent_only.has_progress(),
        "a percent the engine does not hold turns nothing on"
    );
    let remaining_only = session(|c| vec![task(c, None, None, Some(600))]);
    assert!(
        !remaining_only.has_progress(),
        "a remaining without an actual start or finish leaves every cell empty, so the columns stay off"
    );
}
