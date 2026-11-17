//! Every task's dates and float, listed for a program (`Session::results_json`), worked by hand on a
//! Monday-to-Friday 08:00-17:00 calendar (540 working minutes a day) whose day 0 is Monday 2026-01-05.

use crate::common;
use crate::document_round_trip::session_of;

use app::{Session, edit::Edit, load_bytes, meta::TaskMeta};
use engine::{Calendar, Dependency, Limit, LinkType, Schedule, Task, TaskId};
use import::ProjectSelection;
use serde_json::{Value, json};
use timeline::Timeline;
use xer::synth::{Spec, write as synth_xer};

/// A session over `tasks` (minutes, limit, actual start and finish), finish-to-start `links` (pred,
/// succ) and the `(key, code)` of each task.
fn session(
    tasks: &[(i64, Limit, Option<i64>, Option<i64>)],
    links: &[(u32, u32)],
    names: &[(Option<&str>, Option<&str>)],
) -> Session {
    let mut schedule = Schedule::new();
    let days = std::array::from_fn(|day| if day < 5 { vec![(480, 1020)] } else { vec![] });
    let calendar = schedule.add_calendar(Calendar::new(days, &[]).expect("a valid week"));
    for &(duration, limit, actual_start, actual_finish) in tasks {
        let task = Task {
            duration,
            calendar,
            snet: None,
            limit,
            deleted: false,
            actual_start,
            actual_finish,
            remaining: None,
        };
        schedule
            .add_task(task)
            .expect("the calendar is the schedule's");
    }
    for &(pred, succ) in links {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let link = Dependency {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        };
        schedule.add_dependency(link).expect("the tasks exist");
    }
    let solution = schedule.compute().expect("the fixture has no loop");
    let owned = |text: Option<&str>| text.map(str::to_owned);
    let meta = names
        .iter()
        .map(|&(key, code)| TaskMeta {
            key: owned(key),
            code: owned(code),
            ..TaskMeta::default()
        })
        .collect();
    let order = (0..u32::try_from(tasks.len()).expect("a small fixture")).collect();
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, common::clock)
}

fn results(session: &Session) -> Vec<Value> {
    let text = session.results_json().expect("the schedule is solved");
    serde_json::from_str(&text).expect("results_json writes a JSON array")
}

/// Tasks 0 to 2 are the critical chain (a day, two days, a day); 3 (a day) and 4 (half a day) feed
/// task 2 beside task 1: 3 has a day and a half of total float and none free, 4 a day and a half of each.
#[test]
fn every_field_of_a_keyed_schedule_is_the_hand_worked_value() {
    let day = (540, Limit::default(), None, None);
    let tasks = [
        day,
        (1080, Limit::default(), None, None),
        day,
        day,
        (270, Limit::default(), None, None),
    ];
    let names = [
        (Some("k0"), Some("A-1")),
        (Some("k\"1\n"), None),
        (None, None),
        (Some("s"), None),
        (Some("m"), None),
    ];
    let session = session(&tasks, &[(0, 1), (1, 2), (3, 4), (4, 2)], &names);
    let expected = json!([
        {"task": 0, "key": "k0", "code": "A-1", "start": "2026-01-05T08:00", "finish": "2026-01-05T17:00", "lateStart": "2026-01-05T08:00", "lateFinish": "2026-01-05T17:00", "float": 0, "freeFloat": 0, "critical": true, "finished": false},
        {"task": 1, "key": "k\"1\n", "code": null, "start": "2026-01-06T08:00", "finish": "2026-01-07T17:00", "lateStart": "2026-01-06T08:00", "lateFinish": "2026-01-07T17:00", "float": 0, "freeFloat": 0, "critical": true, "finished": false},
        {"task": 2, "key": null, "code": null, "start": "2026-01-08T08:00", "finish": "2026-01-08T17:00", "lateStart": "2026-01-08T08:00", "lateFinish": "2026-01-08T17:00", "float": 0, "freeFloat": 0, "critical": true, "finished": false},
        {"task": 3, "key": "s", "code": null, "start": "2026-01-05T08:00", "finish": "2026-01-05T17:00", "lateStart": "2026-01-06T12:30", "lateFinish": "2026-01-07T12:30", "float": 1.5, "freeFloat": 0, "critical": false, "finished": false},
        {"task": 4, "key": "m", "code": null, "start": "2026-01-06T08:00", "finish": "2026-01-06T12:30", "lateStart": "2026-01-07T12:30", "lateFinish": "2026-01-07T17:00", "float": 1.5, "freeFloat": 1.5, "critical": false, "finished": false}
    ]);
    assert_eq!(Value::Array(results(&session)), expected);
}

/// A task that must finish by Monday 17:00 but takes two days: its late start is the Friday before,
/// a working day earlier than its early start, so its float is minus one day; its free float is 0.
#[test]
fn a_late_limit_gives_a_negative_float() {
    let limit = Limit {
        fnlt: Some(1020),
        ..Limit::default()
    };
    let rows = results(&session(
        &[(1080, limit, None, None)],
        &[],
        &[(Some("late"), None)],
    ));
    let expected = json!({
        "task": 0, "key": "late", "code": null, "start": "2026-01-05T08:00", "finish": "2026-01-06T17:00",
        "lateStart": "2026-01-02T08:00", "lateFinish": "2026-01-05T17:00", "float": -1, "freeFloat": 0, "critical": true, "finished": false,
    });
    assert_eq!(rows, [expected]);
}

/// A task with an actual finish has no float and is not critical, whatever follows it.
#[test]
fn a_finished_task_reads_zero_float_and_finished() {
    let tasks = [
        (540, Limit::default(), Some(480), Some(1020)),
        (540, Limit::default(), None, None),
    ];
    let rows = results(&session(&tasks, &[(0, 1)], &[(None, None), (None, None)]));
    let read = |row: usize| {
        ["float", "freeFloat", "finished", "critical"].map(|field| rows[row][field].clone())
    };
    assert_eq!(read(0), [json!(0), json!(0), json!(true), json!(false)]);
    assert_eq!(read(1), [json!(0), json!(0), json!(false), json!(true)]);
}

#[test]
fn a_schedule_with_no_tasks_gives_an_empty_list() {
    assert_eq!(results(&session(&[], &[], &[])), Vec::<Value>::new());
}

/// A summary and a deleted task are not listed; the undo brings the deleted task back.
#[test]
fn a_summary_and_a_deleted_task_are_absent() {
    let xer = synth_xer(&Spec::with_activities(7, 6));
    let loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).expect("an XER loads");
    let slots = u32::try_from(loaded.meta.len()).expect("a small file");
    let leaves: Vec<u64> = (0..slots)
        .filter(|&t| !loaded.outline.is_summary(TaskId(t)))
        .map(u64::from)
        .collect();
    assert!(leaves.len() < loaded.meta.len(), "the file has a summary");
    let mut session = session_of(loaded);
    let listed = |session: &Session| -> Vec<u64> {
        results(session)
            .iter()
            .map(|row| row["task"].as_u64().expect("a slot"))
            .collect()
    };
    assert_eq!(listed(&session), leaves);
    let deleted = TaskId(u32::try_from(leaves[1]).expect("a small slot"));
    session
        .edit(Edit::Delete { task: deleted })
        .expect("a leaf deletes");
    assert_eq!(listed(&session), [&leaves[..1], &leaves[2..]].concat());
    assert!(session.undo().expect("the delete is undone"));
    assert_eq!(listed(&session), leaves);
}
