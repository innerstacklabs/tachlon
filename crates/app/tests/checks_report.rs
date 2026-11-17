//! The schedule checks as one JSON report, against a hand-counted six-task fixture (days are 480
//! working minutes).
//!
//! ```text
//! 0 -> 1  FS  lag    0     the chain 0, 1, 2 is 1d, 1d, 1d
//! 1 -> 2  SS  lag +480
//! 0 -> 2  FF  lag -240
//! 3 leaf, 46d, alone       sets the project finish; longer than 44d
//! 4 leaf, 1d, alone        float 45d, more than 44d
//! 5 leaf, 1d, alone        actual start on day 8, so it is pinned there: float 37d
//! ```
//!
//! Task 0 has no predecessor, 2 no successor, and 3, 4 and 5 have neither: five unfinished leaves
//! with missing logic. There are 3 links: 1 finish-to-start, 1 lead, 1 lag. High duration is
//! task 3. High float is task 4 (the chain has 43d of float, task 5 has 37d). With a data date of
//! day 7, task 5 has an actual start on day 8: one invalid date.

use crate::common;

use app::baseline::{Baseline, BaselineTask};
use app::{Session, check_filter::CheckKind, checks::report, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, DAY, Dependency, Instant, LinkType, Minutes, Schedule, Task, TaskId};
use serde_json::{Value, json};
use timeline::Timeline;

const DAY_MINUTES: i64 = 480;

fn link(pred: u32, succ: u32, kind: LinkType, lag: Minutes) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind,
        lag,
    }
}

fn session(
    days: &[i64],
    links: &[Dependency],
    started: Option<(u32, Instant)>,
    finished: Option<(u32, Instant)>,
    data_date: Option<Instant>,
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
    for (index, days) in days.iter().enumerate() {
        schedule
            .add_task(Task {
                duration: days * DAY_MINUTES,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: started.and_then(|(task, at)| (task as usize == index).then_some(at)),
                actual_finish: finished
                    .and_then(|(task, at)| (task as usize == index).then_some(at)),
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    for dep in links {
        schedule.add_dependency(*dep).expect("tasks exist");
    }
    schedule.set_data_date(data_date);
    let solution = schedule.compute().expect("the fixture has no loop");
    let metas: Vec<_> = (0..days.len())
        .map(|i| TaskMeta {
            name: format!("task {i}"),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        })
        .collect();
    let wbs: Vec<_> = (0..days.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&metas, &wbs, &schedule).expect("valid outline");
    let timeline = Timeline::new((0..days.len() as u32).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, metas, clock)
        .with_data_date(data_date)
        .with_outline(outline)
        .expect("outline matches the schedule")
}

fn fixture(data_date: Option<Instant>) -> Session {
    session(
        &[1, 1, 1, 46, 1, 1],
        &[
            link(0, 1, LinkType::FS, 0),
            link(1, 2, LinkType::SS, 480),
            link(0, 2, LinkType::FF, -240),
        ],
        Some((5, 8 * DAY)),
        None,
        data_date,
    )
}

#[test]
fn the_report_lists_every_count() {
    assert_eq!(
        report::json(&fixture(Some(7 * DAY))),
        concat!(
            r#"{"missing_logic":5,"open_starts":4,"open_ends":4,"leads":1,"lags":1,"#,
            r#""links":3,"finish_to_start":1,"#,
            r#""high_float":1,"high_duration":1,"hard_constraints":0,"negative_float":0,"#,
            r#""invalid_dates":1,"missing_resources":null,"#,
            r#""missed":0,"execution_index":null}"#
        )
    );
}

/// Four tasks, one resource name (on task 0): task 1 has none and counts; task 2 has none but is
/// finished, and task 3 is a milestone with none, so neither counts.
#[test]
fn missing_resources_is_a_count_once_the_file_names_a_resource() {
    let session = session(&[1, 1, 1, 0], &[], None, Some((2, 3 * DAY)), None).with_resources(vec![
        vec!["Crew A".to_string()],
        vec![],
        vec![],
        vec![],
    ]);

    let json = report::json(&session);

    assert!(
        json.contains(r#""invalid_dates":null,"missing_resources":1,"missed":"#),
        "{json}"
    );
}

#[test]
fn invalid_dates_is_null_without_a_data_date() {
    let json = report::json(&fixture(None));

    assert!(json.contains(r#""invalid_dates":null,"#), "{json}");
    assert!(!json.contains(r#""invalid_dates":0"#), "{json}");
}

#[test]
fn a_schedule_with_no_links_reports_zero_links() {
    let json = report::json(&session(&[1, 1], &[], None, None, None));

    assert_eq!(
        json,
        concat!(
            r#"{"missing_logic":2,"open_starts":2,"open_ends":2,"leads":0,"lags":0,"#,
            r#""links":0,"finish_to_start":0,"#,
            r#""high_float":0,"high_duration":0,"hard_constraints":0,"negative_float":0,"#,
            r#""invalid_dates":null,"missing_resources":null,"#,
            r#""missed":0,"execution_index":null}"#
        )
    );
}

/// Task 0 feeds tasks 1 and 2: one task has no predecessor and two have no successor, so the two
/// counts differ and a swap shows.
#[test]
fn open_starts_and_open_ends_are_counted_apart() {
    let json = report::json(&session(
        &[1, 1, 1],
        &[link(0, 1, LinkType::FS, 0), link(0, 2, LinkType::FS, 0)],
        None,
        None,
        None,
    ));

    assert!(
        json.starts_with(r#"{"missing_logic":3,"open_starts":1,"open_ends":2,"leads":"#),
        "{json}"
    );
}

/// Data date day 7; baselined to finish on day 3 (finished on time), 5 (not finished: missed) and
/// 9 (not due yet, not counted): one missed, and 1 of the 2 due have finished.
#[test]
fn the_report_lists_missed_and_the_index() {
    let tasks = [3, 5, 9].map(|ef| {
        Some(BaselineTask {
            es: 0,
            ef: ef * DAY,
            float: None,
        })
    });
    let session = session(&[1, 1, 1], &[], None, Some((0, 3 * DAY)), Some(7 * DAY))
        .with_baseline(Baseline::from_parts(tasks.to_vec(), 20 * DAY));

    let json = report::json(&session);

    assert!(
        json.ends_with(r#""missed":1,"execution_index":0.5}"#),
        "{json}"
    );
    assert_eq!(held(&health(&session), "missed"), [1]);
}

fn health(session: &Session) -> Value {
    serde_json::from_str(&session.health()).expect("health answers JSON")
}

fn held(health: &Value, key: &str) -> Vec<u64> {
    let tasks = health["counts"][key]["tasks"].as_array().expect("a count");
    tasks
        .iter()
        .map(|t| t["task"].as_u64().expect("a number"))
        .collect()
}

/// The fixture's counts, hand-listed: task 0 has no predecessor, 2 no successor, 3, 4 and 5 neither.
#[test]
fn health_lists_the_tasks_behind_each_count() {
    let answer = health(&fixture(Some(7 * DAY)));
    for (key, tasks) in [
        ("missing_logic", vec![0, 2, 3, 4, 5]),
        ("open_starts", vec![0, 3, 4, 5]),
        ("open_ends", vec![2, 3, 4, 5]),
        ("high_float", vec![4]),
        ("high_duration", vec![3]),
        ("invalid_dates", vec![5]),
        ("missed", vec![]),
        ("hard_constraints", vec![]),
        ("negative_float", vec![]),
    ] {
        assert_eq!(held(&answer, key), tasks, "{key}");
        assert_eq!(answer["counts"][key]["count"], tasks.len(), "{key}");
    }
    let first = &answer["counts"]["high_duration"]["tasks"][0];
    assert_eq!(first, &json!({"task": 3, "id": "", "name": "task 3"}));
    assert!(
        answer["counts"]["missing_resources"].is_null(),
        "no file resource"
    );
    assert!(
        health(&fixture(None))["counts"]["invalid_dates"].is_null(),
        "no data date"
    );
}

#[test]
fn health_lists_the_tasks_missing_a_resource() {
    let named = vec![vec!["Crew A".to_string()], vec![], vec![], vec![]];
    let session = session(&[1, 1, 1, 0], &[], None, Some((2, 3 * DAY)), None).with_resources(named);
    assert_eq!(held(&health(&session), "missing_resources"), [1]);
}

/// What the Show filter keeps for a link check is what its health count lists.
#[test]
fn health_lists_what_the_check_filter_keeps() {
    let mut session = fixture(Some(7 * DAY));
    let counts = health(&session);
    for (kind, key) in [
        (CheckKind::MissingLogic, "missing_logic"),
        (CheckKind::OpenStart, "open_starts"),
        (CheckKind::OpenEnd, "open_ends"),
        (CheckKind::HighFloat, "high_float"),
        (CheckKind::HighDuration, "high_duration"),
        (CheckKind::InvalidDates, "invalid_dates"),
    ] {
        assert!(session.set_check_filter(Some(kind)));
        let mut kept: Vec<u64> = (session.timeline().order().iter())
            .map(|&n| n.into())
            .collect();
        kept.sort_unstable();
        assert_eq!(held(&counts, key), kept, "{key}");
    }
}

#[test]
fn health_lists_fifty_tasks_and_counts_them_all() {
    let answer = health(&session(&[1; 60], &[], None, None, None));
    assert_eq!(answer["counts"]["open_ends"]["count"], 60);
    assert_eq!(held(&answer, "open_ends"), (0..50).collect::<Vec<u64>>());
}
