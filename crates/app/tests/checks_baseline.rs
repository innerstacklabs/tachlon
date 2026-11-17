//! The missed-task count and the baseline execution index, each against a hand-counted fixture.
//!
//! The data date is day 10 (days are 480 working minutes). Each task is `baselined early finish
//! / actual finish`:
//!
//! ```text
//! 0 leaf    5d / 5d      finished on time
//! 1 leaf    5d / 7d      finished late: missed
//! 2 leaf    8d / -       unfinished: missed
//! 3 leaf   10d / 9d      baselined on the data date, finished early
//! 4 leaf   11d / -       baselined after the data date: not counted
//! 5 leaf    3d / -       deleted: not counted
//! 6 summary { 7 } 2d / - a summary: not counted
//! 7 leaf    2d / -       unfinished: missed
//! 8 leaf   no entry / -  added since the baseline: not counted
//! ```
//!
//! Five tasks (0, 1, 2, 3, 7) were baselined to finish by then, three of them (0, 1, 3) finished,
//! and three (1, 2, 7) missed.

use crate::common;

use app::baseline::{Baseline, BaselineTask};
use app::{Session, checks::baseline::BaselineChecks, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, DAY, Instant, Schedule, Task, TaskId};
use timeline::Timeline;

const DAY_MINUTES: i64 = 480;

struct Spec {
    baseline_ef: Option<i64>,
    finish: Option<i64>,
    deleted: bool,
}

fn spec(baseline_ef: Option<i64>, finish: Option<i64>) -> Spec {
    Spec {
        baseline_ef,
        finish,
        deleted: false,
    }
}

fn meta(parent: Option<TaskId>, level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    }
}

fn session(
    specs: &[Spec],
    parents: &[Option<u32>],
    data_date: Option<Instant>,
    baselined: bool,
) -> Session {
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
                duration: DAY_MINUTES,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: spec.deleted,
                actual_start: spec.finish.map(|at| at * DAY - DAY),
                actual_finish: spec.finish.map(|at| at * DAY),
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
    let session = Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_data_date(data_date)
        .with_outline(outline)
        .expect("outline matches the schedule");
    if !baselined {
        return session;
    }
    let tasks = specs
        .iter()
        .map(|spec| {
            spec.baseline_ef.map(|ef| BaselineTask {
                es: 0,
                ef: ef * DAY,
                float: None,
            })
        })
        .collect();
    session.with_baseline(Baseline::from_parts(tasks, 20 * DAY))
}

fn specs() -> Vec<Spec> {
    let mut deleted = spec(Some(3), None);
    deleted.deleted = true;
    vec![
        spec(Some(5), Some(5)),
        spec(Some(5), Some(7)),
        spec(Some(8), None),
        spec(Some(10), Some(9)),
        spec(Some(11), None),
        deleted,
        spec(Some(2), None),
        spec(Some(2), None),
        spec(None, None),
    ]
}

const PARENTS: [Option<u32>; 9] = [None, None, None, None, None, None, None, Some(6), None];

fn fixture() -> Session {
    session(&specs(), &PARENTS, Some(10 * DAY), true)
}

#[test]
fn missed_tasks_counts_late_and_unfinished_tasks_baselined_by_the_data_date() {
    let checks = BaselineChecks::of(&fixture());

    assert_eq!(checks.missed_tasks, 3);
}

#[test]
fn the_index_is_finished_over_baselined_by_the_data_date() {
    let checks = BaselineChecks::of(&fixture());

    assert_eq!((checks.finished, checks.baselined_to_finish), (3, 5));
    assert_eq!(checks.execution_index(), Some(0.6));
}

#[test]
fn no_baseline_or_no_data_date_gives_no_index() {
    let no_baseline = session(&specs(), &PARENTS, Some(10 * DAY), false);
    let no_data_date = session(&specs(), &PARENTS, None, true);

    for session in [no_baseline, no_data_date] {
        let checks = BaselineChecks::of(&session);
        assert_eq!(checks.missed_tasks, 0);
        assert_eq!(checks.execution_index(), None);
    }
}

#[test]
fn nothing_baselined_by_the_data_date_gives_no_index() {
    let late = [spec(Some(30), None)];
    let session = session(&late, &[None], Some(10 * DAY), true);

    let checks = BaselineChecks::of(&session);

    assert_eq!(checks.missed_tasks, 0);
    assert_eq!(checks.execution_index(), None);
}
