//! Why a task starts when it does. Tasks 0 to 2 are a chain of eight working hours each on a
//! Monday-to-Friday calendar (08:00-12:00, 13:00-17:00), so they run Monday to Wednesday; task 3 is
//! eight hours that start Monday and feed task 2 and task 4, which follows it on Tuesday. Day 0 is
//! Monday 2026-01-05. Task 3 ends Monday, a day before task 2 can start, so it drives nothing there
//! and has a day of float.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::collapse;
use app::{Session, meta::TaskMeta};
use engine::{Dependency, LinkType, TaskId};
use serde_json::{Value, json};
use timeline::Timeline;

const HOUR: i64 = 60;

fn session() -> Session {
    let mut schedule = collapse::schedule_with_tasks(5);
    for i in 0..5 {
        schedule.set_duration(TaskId(i), 8 * HOUR).unwrap();
    }
    for (pred, succ) in [(0, 1), (1, 2), (3, 2), (3, 4)] {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let kind = LinkType::FS;
        let link = Dependency {
            pred,
            succ,
            kind,
            lag: 0,
        };
        schedule.add_link(link).unwrap();
    }
    let solution = schedule.compute().unwrap();
    let meta = (0..5).map(|i| TaskMeta {
        name: format!("Task \"{i}\""),
        code: (i != 3).then(|| format!("A10{i}0")),
        ..TaskMeta::default()
    });
    let timeline = Timeline::new((0..5).collect(), 1_000.0, 400.0, 1.0);
    Session::new(
        schedule,
        solution,
        timeline,
        meta.collect(),
        crate::common::clock,
    )
}

fn ask(session: &Session, task: u32) -> Value {
    serde_json::from_str(&session.explain(task)).unwrap()
}

fn chain(answer: &Value) -> Vec<u64> {
    let list = answer["chain"].as_array().unwrap();
    list.iter().map(|l| l["task"].as_u64().unwrap()).collect()
}

#[test]
fn the_chain_is_the_driving_predecessors_nearest_first_with_names_ids_and_float() {
    let live = session();
    let answer = ask(&live, 2);
    // Task 2 starts Wednesday because task 1 ends Tuesday; task 3 ends Monday and drives nothing.
    assert_eq!(answer["start"], "2026-01-07 08:00");
    assert_eq!(chain(&answer), [1, 0]);
    assert_eq!(answer["capped"], false);
    assert_eq!(
        answer["chain"][0],
        json!({"task": 1, "id": "A1010", "name": "Task \"1\"", "float": "0d"})
    );
    // The first step is what the engine says drives it.
    let engine = live
        .schedule()
        .driving_predecessors(live.solution(), TaskId(2));
    assert_eq!(engine, [TaskId(1)]);
    // Task 4 follows task 3, which has no activity ID and a day of float.
    let off = ask(&live, 4);
    assert_eq!(off["start"], "2026-01-06 08:00");
    assert_eq!(
        off["chain"],
        json!([{"task": 3, "id": "", "name": "Task \"3\"", "float": "1d"}])
    );
    assert_eq!(chain(&ask(&live, 0)), Vec::<u64>::new());
}

#[test]
fn asking_changes_nothing_and_an_unknown_task_is_refused_with_a_reason() {
    let live = session();
    let (solution, history, stamp) = (live.solution().clone(), live.history_len(), live.stamp());
    let first = live.explain(2);
    assert_eq!(live.explain(2), first);
    assert!(live.solution() == &solution && live.history_len() == history && live.stamp() == stamp);
    let refused = ask(&live, 99);
    assert_eq!(refused["ok"], false);
    assert_eq!(refused["reason"], "unknownTask");
    assert_ne!(refused["message"].as_str().unwrap(), "");
}
