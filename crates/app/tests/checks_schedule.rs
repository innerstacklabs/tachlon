//! The DCMA float, duration and date counts, each against a hand-counted fixture.
//!
//! The long-task fixture (days are 480 working minutes):
//!
//! ```text
//! 0 leaf, 100d, the longest task: it sets the project finish
//! 1 summary { 2 }, 50d, alone
//! 2 leaf, 1d, alone                  float 99d
//! 3 leaf, 56d, alone                 float 44d
//! 4 leaf, 55d, alone                 float 45d
//! 5 leaf, 45d, finished              float 0
//! 6 leaf, 99d, deleted
//! 7 leaf, 44d, alone                 float 56d
//! ```
//!
//! High float is more than 44d on an unfinished leaf: tasks 2, 4 and 7 (3 is on the boundary, 1
//! is a summary, 5 is finished). High duration is a live leaf longer than 44d: tasks 0, 3, 4 and
//! 5 (7 is on the boundary, 1 is a summary, 6 is deleted).

use crate::common;

use app::{Session, checks::schedule::ScheduleChecks, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, DAY, Instant, Limit, Pin, Schedule, Task, TaskId};
use timeline::Timeline;

const DAY_MINUTES: i64 = 480;

fn meta(parent: Option<TaskId>, level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    }
}

struct Spec {
    days: i64,
    start: Option<Instant>,
    finish: Option<Instant>,
    deleted: bool,
    limit: Limit,
    snet: Option<Instant>,
}

fn spec(days: i64) -> Spec {
    Spec {
        days,
        start: None,
        finish: None,
        deleted: false,
        limit: Limit::default(),
        snet: None,
    }
}

/// A task with `limit` that starts no earlier than day 3, so a late limit before then has no date to meet.
fn limited(days: i64, limit: Limit) -> Spec {
    Spec {
        limit,
        snet: Some(3 * DAY),
        ..spec(days)
    }
}

fn session(specs: &[Spec], parents: &[Option<u32>], data_date: Option<Instant>) -> Session {
    let day = vec![(480, 720), (780, 1020)];
    let calendar = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    for spec in specs {
        schedule
            .add_task(Task {
                duration: spec.days * DAY_MINUTES,
                calendar,
                snet: spec.snet,
                limit: spec.limit,
                deleted: spec.deleted,
                actual_start: spec.start,
                actual_finish: spec.finish,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule.set_data_date(data_date);
    let solution = schedule.compute().expect("the fixture has no loop");
    let metas: Vec<_> = parents
        .iter()
        .map(|parent| match parent {
            Some(p) => meta(Some(TaskId(*p)), 2),
            None => meta(None, 1),
        })
        .collect();
    let wbs: Vec<_> = (0..specs.len() as u32)
        .filter(|&index| !specs[index as usize].deleted)
        .map(TaskId)
        .collect();
    let outline = Outline::new(&metas, &wbs, &schedule).expect("valid outline");
    let timeline = Timeline::new((0..specs.len() as u32).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_data_date(data_date)
        .with_outline(outline)
        .expect("outline matches the schedule")
}

fn long_tasks() -> Session {
    let mut finished = spec(45);
    finished.start = Some(0);
    finished.finish = Some(45 * DAY_MINUTES);
    let mut deleted = spec(99);
    deleted.deleted = true;
    session(
        &[
            spec(100),
            spec(50),
            spec(1),
            spec(56),
            spec(55),
            finished,
            deleted,
            spec(44),
        ],
        &[None, None, Some(1), None, None, None, None, None],
        None,
    )
}

#[test]
fn high_float_counts_unfinished_leaves_over_44_working_days() {
    let checks = ScheduleChecks::of(&long_tasks());

    assert_eq!(checks.high_float, 3);
}

#[test]
fn high_duration_counts_leaves_over_44_working_days() {
    let checks = ScheduleChecks::of(&long_tasks());

    assert_eq!(checks.high_duration, 4);
}

/// The data date is the second Monday (day 7). Task 0 started before it; 1 started after it; 2
/// finished after it; 3 did both (it counts once); 4 has no actuals; 5 finished on the data date.
/// Three tasks have an actual after the data date.
#[test]
fn invalid_dates_counts_tasks_with_an_actual_after_the_data_date() {
    let data_date = 7 * DAY;
    let mut started_before = spec(1);
    started_before.start = Some(0);
    let mut started_after = spec(1);
    started_after.start = Some(8 * DAY);
    let mut finished_after = spec(1);
    finished_after.start = Some(0);
    finished_after.finish = Some(9 * DAY);
    let mut both_after = spec(1);
    both_after.start = Some(8 * DAY);
    both_after.finish = Some(9 * DAY);
    let mut finished_on = spec(1);
    finished_on.start = Some(0);
    finished_on.finish = Some(data_date);
    let session = session(
        &[
            started_before,
            started_after,
            finished_after,
            both_after,
            spec(1),
            finished_on,
        ],
        &[None; 6],
        Some(data_date),
    );

    assert_eq!(ScheduleChecks::of(&session).invalid_dates, 3);
}

#[test]
fn no_data_date_means_no_invalid_dates() {
    let mut started = spec(1);
    started.start = Some(8 * DAY);
    let session = session(&[started], &[None], None);

    assert_eq!(ScheduleChecks::of(&session).invalid_dates, 0);
}
fn limit(set: impl FnOnce(&mut Limit)) -> Limit {
    let mut limit = Limit::default();
    set(&mut limit);
    limit
}

/// A started task: it has an actual start, so it ignores a start limit.
fn started(mut spec: Spec) -> Spec {
    spec.start = Some(0);
    spec
}

/// A finished task: it ignores every limit.
fn finished(mut spec: Spec) -> Spec {
    spec.finish = Some(DAY_MINUTES);
    started(spec)
}

/// Hard constraints are the late limits and the pins an unfinished leaf still obeys:
///
/// ```text
/// 0 start-no-later-than                  counts
/// 1 finish-no-later-than                 counts
/// 2 must start on                        counts
/// 3 must finish on, started              counts (a finish pin binds a started task)
/// 4 must start on, started               no: a started task ignores a start pin
/// 5 finish-no-earlier-than only          no: it is a floor, not a hold
/// 6 no limit                             no
/// 7 finish-no-later-than, finished       no
/// 8 finish-no-later-than, deleted        no
/// ```
#[test]
fn hard_constraints_count_the_limits_an_unfinished_leaf_obeys() {
    let at = 10 * DAY;
    let mut deleted = limited(1, limit(|l| l.fnlt = Some(at)));
    deleted.deleted = true;
    let session = session(
        &[
            limited(1, limit(|l| l.snlt = Some(at))),
            limited(1, limit(|l| l.fnlt = Some(at))),
            limited(1, limit(|l| l.pin = Some(Pin::Start(at)))),
            started(limited(1, limit(|l| l.pin = Some(Pin::Finish(at))))),
            started(limited(1, limit(|l| l.pin = Some(Pin::Start(at))))),
            limited(1, limit(|l| l.fnet = Some(at))),
            spec(1),
            finished(limited(1, limit(|l| l.fnlt = Some(at)))),
            deleted,
        ],
        &[None; 9],
        None,
    );

    assert_eq!(ScheduleChecks::of(&session).hard_constraints, 4);
}

/// Every limited task starts no earlier than day 3, so a late limit before that has no date to
/// meet and the float is negative: tasks 0, 1 and 5 count. Task 2 is finished (float 0), summary
/// 3 is not counted, and its child 4 is free.
#[test]
fn negative_float_counts_unfinished_leaves_a_late_limit_cannot_meet() {
    let session = session(
        &[
            limited(3, limit(|l| l.fnlt = Some(DAY))),
            limited(3, limit(|l| l.fnlt = Some(2 * DAY))),
            finished(limited(1, limit(|l| l.fnlt = Some(DAY)))),
            limited(3, limit(|l| l.fnlt = Some(DAY))),
            spec(1),
            limited(2, limit(|l| l.pin = Some(Pin::Finish(DAY)))),
        ],
        &[None, None, None, None, Some(3), None],
        None,
    );

    assert_eq!(ScheduleChecks::of(&session).negative_float, 3);
}
