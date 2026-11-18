//! Typing a percent complete (gs#703): the typed percent becomes the remaining duration, a task
//! with no actual start takes one (except for 0, which leaves it unstarted), 100 finishes the task, 0 keeps the start and leaves the whole
//! duration remaining, and undo puts the three progress fields back. The dates are worked out by
//! hand: a task of four working days (32 hours) with a successor of one day, on a Monday-to-Friday
//! calendar with 08:00-12:00 and 13:00-17:00 shifts; day 0 is the origin Monday, so Monday 08:00
//! is minute 480.

use crate::common;

use app::{Session, edit::EditError, meta::TaskMeta};
use common::{clock, collapse::standard_calendar};
use engine::{Dependency, Instant, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;
const FIRST: TaskId = TaskId(0);

fn at(day: i64, hour: i64) -> Instant {
    day * DAY + hour * HOUR
}

/// A four-day task and a one-day task after it, finish to start.
fn pair() -> Schedule {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let ids: Vec<TaskId> = [32, 8]
        .iter()
        .map(|hours| {
            schedule
                .add_task(Task {
                    duration: hours * HOUR,
                    calendar,
                    snet: None,
                    limit: engine::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                })
                .expect("the calendar belongs to the schedule")
        })
        .collect();
    schedule
        .add_link(Dependency {
            pred: ids[0],
            succ: ids[1],
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a pair has no loop");
    schedule
}

fn session() -> Session {
    let mut schedule = pair();
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 2],
        clock,
    )
}

fn fields(s: &Session) -> (Option<Instant>, Option<Instant>, Option<i64>) {
    let t = &s.schedule().tasks()[0];
    (t.actual_start, t.actual_finish, t.remaining)
}

#[test]
fn typing_75_on_a_four_day_task_leaves_one_day_remaining_and_undo_restores_all_three_fields() {
    let mut s = session();
    let before = s.solution().clone();
    assert_eq!(fields(&s), (None, None, None));
    assert_eq!(
        s.solution().dates[1].es,
        at(4, 8),
        "the second task waits four days"
    );

    s.set_percent_text(FIRST, "75%").expect("75% applies");
    assert_eq!(
        fields(&s),
        (Some(at(0, 8)), None, Some(8 * HOUR)),
        "a task with no start takes its early start; a quarter remains"
    );
    assert_eq!(s.task_percent(0), "75%");
    assert_eq!(
        s.solution().dates[1].es,
        at(1, 8),
        "the bar slides: the successor follows"
    );
    let mut fresh = pair();
    fresh.set_actual_start(FIRST, Some(at(0, 8))).unwrap();
    fresh.set_remaining(FIRST, Some(8 * HOUR)).unwrap();
    assert_eq!(*s.solution(), fresh.compute().expect("computes"));
    assert!(s.move_report().is_some(), "the what-moved note is there");

    assert!(s.undo().unwrap());
    assert_eq!(fields(&s), (None, None, None), "all three fields are back");
    assert_eq!(*s.solution(), before);
    assert!(s.redo().unwrap());
    assert_eq!(fields(&s), (Some(at(0, 8)), None, Some(8 * HOUR)));
}

#[test]
fn a_bare_number_is_a_percent_and_a_started_task_keeps_its_start() {
    let mut s = session();
    s.set_progress_field_text(FIRST, "actual_start", "2026-01-06 13:00")
        .expect("a start with a time of day applies");
    let start = s.schedule().tasks()[0].actual_start;
    s.set_percent_text(FIRST, " 50 ").expect("50 applies");
    assert_eq!(fields(&s), (start, None, Some(16 * HOUR)));
    assert_eq!(s.task_percent(0), "50%");
}

#[test]
fn typing_100_finishes_the_task_and_undo_puts_it_back() {
    let mut s = session();
    s.set_percent_text(FIRST, "50").expect("applies");
    let was = fields(&s);
    s.set_percent_text(FIRST, "100").expect("100 finishes");
    let (start, finish, remaining) = fields(&s);
    assert_eq!(start, was.0);
    assert_eq!(finish, Some(s.solution().dates[0].ef));
    assert_eq!(remaining, Some(0));
    assert!(s.solution().dates[0].finished);
    assert_eq!(s.task_percent(0), "100%");
    s.undo().unwrap();
    assert_eq!(fields(&s), was);
}

#[test]
fn typing_0_keeps_the_start_and_leaves_the_whole_duration_remaining() {
    let mut s = session();
    s.set_percent_text(FIRST, "100").expect("applies");
    s.set_percent_text(FIRST, "0").expect("0 applies");
    assert_eq!(fields(&s), (Some(at(0, 8)), None, Some(32 * HOUR)));
    assert_eq!(s.task_percent(0), "0%");
    assert!(
        !s.solution().dates[0].finished,
        "typing 0 reopens a finished task"
    );
}

#[test]
fn typing_0_on_a_task_that_has_not_started_leaves_it_unstarted_in_the_cell_and_the_dialog() {
    let mut s = session();
    s.set_percent_text(FIRST, "0").expect("0 applies");
    assert_eq!(fields(&s), (None, None, None), "the cell");
    s.set_task_fields(FIRST, &[("percent", "0")])
        .expect("0 applies");
    assert_eq!(fields(&s), (None, None, None), "the dialog");
    assert_eq!(
        s.history_len().0,
        0,
        "a 0 that changes nothing is no undo step"
    );
    s.set_percent_text(FIRST, "50").expect("50 starts it");
    assert_eq!(fields(&s), (Some(at(0, 8)), None, Some(16 * HOUR)));
    s.set_percent_text(FIRST, "0").expect("0 applies");
    assert_eq!(
        fields(&s),
        (Some(at(0, 8)), None, Some(32 * HOUR)),
        "a started task keeps its start"
    );
}

#[test]
fn a_typed_percent_that_puts_an_actual_after_the_data_date_is_refused() {
    let mut s = session().with_data_date(Some(at(0, 0)));
    let before = s.solution().clone();
    for percent in ["50", "100"] {
        assert_eq!(
            s.set_percent_text(FIRST, percent),
            Err(EditError::ActualAfterDataDate),
            "{percent}"
        );
        assert_eq!(
            s.plan_percent_text(FIRST, percent),
            Err(EditError::ActualAfterDataDate),
            "{percent} planned"
        );
    }
    assert_eq!(fields(&s), (None, None, None));
    assert_eq!(*s.solution(), before);
    assert_eq!(s.history_len().0, 0, "a refused entry is no undo step");
}

#[test]
fn a_refused_entry_changes_nothing_and_says_why() {
    let mut s = session();
    let before = s.solution().clone();
    for text in ["101", "abc", "", "-5", "7.5", "%", "1e2"] {
        assert_eq!(
            s.set_percent_text(FIRST, text),
            Err(EditError::BadPercent),
            "{text:?}"
        );
    }
    assert_eq!(fields(&s), (None, None, None));
    assert_eq!(*s.solution(), before);
    assert_eq!(s.history_len().0, 0, "no undo step");
    assert_eq!(
        EditError::BadPercent.to_string(),
        "Write the percent complete as a whole number from 0 to 100, such as 75 or 75%."
    );
}

#[test]
fn a_summary_is_refused() {
    let mut s = common::collapse::session(60);
    let summary = common::collapse::summaries(&s)[0];
    let before = s.solution().clone();
    let error = s.set_percent_text(summary, "50").unwrap_err();
    assert!(
        matches!(error, EditError::ProgressOnSummary { .. }),
        "{error}"
    );
    assert_eq!(*s.solution(), before);
}

/// A task of three working days (24 hours) with a one-day successor.
fn three_day_pair() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let ids: Vec<TaskId> = [24, 8]
        .iter()
        .map(|hours| {
            schedule
                .add_task(Task {
                    duration: hours * HOUR,
                    calendar,
                    snet: None,
                    limit: engine::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                })
                .expect("the calendar belongs to the schedule")
        })
        .collect();
    schedule
        .add_link(Dependency {
            pred: ids[0],
            succ: ids[1],
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a pair has no loop");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 2],
        clock,
    )
}

#[test]
fn typing_60_on_a_24_hour_task_leaves_whole_hours_so_the_successor_starts_on_the_hour() {
    let mut s = three_day_pair();
    s.set_percent_text(FIRST, "60").expect("60 applies");
    // 24 x 40 / 100 = 9.6 hours: whole hours make it 10 (600 minutes), not 576.
    assert_eq!(fields(&s).2, Some(10 * HOUR));
    assert_eq!(s.task_percent(0), "60%", "the cell reads as typed, not 58%");
    assert_eq!(
        s.solution().dates[0].ef % HOUR,
        0,
        "the finish is on the hour"
    );
    assert_eq!(
        s.solution().dates[1].es % HOUR,
        0,
        "the successor starts on the hour"
    );
}

#[test]
fn a_percent_below_100_never_leaves_nothing_remaining() {
    let mut s = three_day_pair();
    // 24 hours x 1 / 100 = 0.24 hours rounds to 0, but 99% is not finished.
    s.set_percent_text(FIRST, "99").expect("99 applies");
    assert_eq!(fields(&s).2, Some(HOUR));
}

#[test]
fn a_later_change_of_the_remaining_work_goes_back_to_the_derived_percent() {
    let mut s = three_day_pair();
    s.set_percent_text(FIRST, "60").expect("60 applies");
    assert_eq!(
        fields(&s).2,
        Some(10 * HOUR),
        "the typed percent leaves whole hours"
    );
    assert_eq!(s.task_percent(0), "60%");
    s.set_progress_field_text(FIRST, "remaining", "12h")
        .expect("12h applies");
    assert_eq!(s.task_percent(0), "50%", "12 of 24 hours done");
    s.undo().unwrap();
    assert_eq!(
        s.task_percent(0),
        "60%",
        "undo puts the typed remaining, and its percent, back"
    );
}

#[test]
fn a_later_finish_goes_back_to_the_derived_percent() {
    let mut s = three_day_pair();
    s.set_percent_text(FIRST, "60").expect("60 applies");
    assert_eq!(
        fields(&s).2,
        Some(10 * HOUR),
        "the typed percent leaves whole hours"
    );
    assert_eq!(s.task_percent(0), "60%");
    s.set_progress_field_text(FIRST, "actual_finish", "2026-01-07")
        .expect("a finish applies");
    assert_eq!(
        s.task_percent(0),
        "100%",
        "a finished task is 100%, not the 60% typed"
    );
}
