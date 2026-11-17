//! What moves if tasks slip a week. A, B and C (tasks 0 to 2) are a chain of eight working hours
//! each on a Monday-to-Friday calendar (08:00-12:00, 13:00-17:00); D is alone. Day 0 is Monday
//! 2026-01-05.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::collapse;
use app::{Session, WHAT_MOVES_MAX, meta::TaskMeta};
use engine::{Dependency, LinkType, TaskId};
use serde_json::{Value, json};
use timeline::Timeline;

const HOUR: i64 = 60;

pub fn session() -> Session {
    sized(4)
}

/// The fixture with `n` tasks: the chain A, B, C and then tasks alone.
pub fn sized(n: u32) -> Session {
    let mut schedule = collapse::schedule_with_tasks(usize::try_from(n).unwrap());
    for i in 0..n {
        schedule.set_duration(TaskId(i), 8 * HOUR).unwrap();
    }
    for i in 0..2 {
        let (pred, succ) = (TaskId(i), TaskId(i + 1));
        let link = Dependency {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        };
        schedule.add_link(link).unwrap();
    }
    let solution = schedule.compute().unwrap();
    let meta = (0..n).map(|i| TaskMeta {
        name: format!("Task \"{i}\""),
        code: (i != 3).then(|| format!("A10{i}0")),
        color: (i == 2).then_some([42, 157, 143, 255]),
        ..TaskMeta::default()
    });
    let timeline = Timeline::new((0..n).collect(), 1_000.0, 400.0, 1.0);
    Session::new(
        schedule,
        solution,
        timeline,
        meta.collect(),
        crate::common::clock,
    )
}

fn ask(session: &Session, tasks: &[u32]) -> Value {
    serde_json::from_str(&session.what_moves(tasks)).unwrap()
}

fn moved(answer: &Value) -> Vec<u64> {
    let list = answer["moved"].as_array().unwrap();
    list.iter().map(|m| m["task"].as_u64().unwrap()).collect()
}

#[test]
fn a_week_moves_the_task_and_what_follows_it_as_a_full_compute_says() {
    let live = session();
    let answer = ask(&live, &[1]);
    assert_eq!(moved(&answer), [1, 2]);
    // B follows A, so it starts Tuesday 2026-01-06 08:00; seven working days on, Thursday 2026-01-15.
    let (b, c) = (&answer["moved"][0], &answer["moved"][1]);
    assert_eq!(
        b,
        &json!({"task": 1, "id": "A1010", "name": "Task \"1\"", "start_was": "2026-01-06 08:00", "start_now": "2026-01-15 08:00", "finish_was": "2026-01-06 17:00", "finish_now": "2026-01-15 17:00"})
    );
    assert_eq!(c["start_now"], "2026-01-16 08:00");
    assert_eq!(answer["finish_was"], "2026-01-07 17:00");
    assert_eq!(answer["finish_now"], "2026-01-16 17:00");
    // The same edit made to a copy and solved in full; 2026-01-15 08:00 is ten days and eight hours after the origin.
    let mut edited = live.schedule().clone();
    let snet = 10 * 24 * HOUR + 8 * HOUR;
    edited.set_snet(TaskId(1), Some(snet)).unwrap();
    let (was, now) = (&live.solution().dates, edited.compute().unwrap().dates);
    let differ = (0..4).filter(|&i| (was[i].es, was[i].ef) != (now[i].es, now[i].ef));
    assert_eq!(differ.map(|i| i as u64).collect::<Vec<_>>(), moved(&answer));
    let two = ask(&live, &[1, 3]);
    assert_eq!(moved(&two), [1, 2, 3], "D slips with B and C");
}

#[test]
fn asking_changes_nothing_and_asking_again_answers_the_same() {
    let live = session();
    let (solution, history, stamp) = (live.solution().clone(), live.history_len(), live.stamp());
    let first = live.what_moves(&[1, 3]);
    let answer: Value = serde_json::from_str(&first).unwrap();
    assert!(
        answer["ok"] == true && !moved(&answer).is_empty(),
        "a slip was calculated: {first}"
    );
    assert_eq!(live.what_moves(&[1, 3]), first);
    assert!(live.solution() == &solution && live.history_len() == history && live.stamp() == stamp);
    assert!(live.schedule().tasks().iter().all(|t| t.snet.is_none()));
}

#[test]
fn an_unknown_started_or_summary_task_and_too_many_are_refused_with_a_reason() {
    let outline = collapse::session(60);
    let summary = collapse::summaries(&outline)[0].0;
    let reason = |s: &Session, tasks: &[u32]| {
        let answer = ask(s, tasks);
        assert_ne!(answer["message"].as_str().unwrap(), "");
        answer["reason"].as_str().unwrap().to_string()
    };
    let (live, mut started) = (session(), session());
    started
        .set_progress_field_text(TaskId(3), "actual_start", "2026-01-05")
        .unwrap();
    assert_eq!(reason(&live, &[99]), "unknownTask");
    assert_eq!(reason(&started, &[3]), "hasActuals");
    assert_eq!(reason(&live, &[0; WHAT_MOVES_MAX + 1]), "tooMany");
    assert_eq!(reason(&outline, &[summary]), "summary");
    assert_eq!(
        ask(&live, &[0; WHAT_MOVES_MAX])["ok"],
        true,
        "ten is the most one ask takes"
    );
}

fn ask_longer(session: &Session, task: u32, days: u32) -> Value {
    serde_json::from_str(&session.what_moves_longer(task, days)).unwrap()
}

#[test]
fn days_longer_moves_what_a_full_compute_of_the_longer_task_says() {
    let live = session();
    let answer = ask_longer(&live, 1, 2);
    assert_eq!(moved(&answer), [1, 2]);
    let (b, c) = (&answer["moved"][0], &answer["moved"][1]);
    assert_eq!(b["start_now"], b["start_was"]);
    assert_eq!(b["finish_now"], "2026-01-08 17:00");
    assert_eq!(c["start_now"], "2026-01-09 08:00");
    assert_eq!(answer["finish_was"], "2026-01-07 17:00");
    assert_eq!(answer["finish_now"], "2026-01-09 17:00");
    let mut edited = live.schedule().clone();
    edited.set_duration(TaskId(1), 24 * HOUR).unwrap();
    let (was, now) = (&live.solution().dates, edited.compute().unwrap().dates);
    let differ = (0..4).filter(|&i| (was[i].es, was[i].ef) != (now[i].es, now[i].ef));
    assert_eq!(differ.map(|i| i as u64).collect::<Vec<_>>(), moved(&answer));
}

#[test]
fn asking_longer_changes_nothing_and_refuses_what_a_slip_refuses() {
    let outline = collapse::session(60);
    let summary = collapse::summaries(&outline)[0].0;
    let (live, mut started) = (session(), session());
    started
        .set_progress_field_text(TaskId(3), "actual_start", "2026-01-05")
        .unwrap();
    let (solution, history, stamp) = (live.solution().clone(), live.history_len(), live.stamp());
    let first = live.what_moves_longer(1, 3);
    assert_eq!(live.what_moves_longer(1, 3), first);
    assert!(live.solution() == &solution && live.history_len() == history && live.stamp() == stamp);
    assert_eq!(live.schedule().tasks()[1].duration, 8 * HOUR);
    let reason = |s: &Session, task: u32| {
        let answer = ask_longer(s, task, 1);
        answer["reason"].as_str().unwrap().to_string()
    };
    assert_eq!(reason(&live, 99), "unknownTask");
    assert_eq!(reason(&started, 3), "hasActuals");
    assert_eq!(reason(&outline, summary), "summary");
    // The longest a task of 8 hours can run is 100 years of 366 days: 109,800 working days.
    assert_eq!(ask_longer(&live, 1, 109_799)["ok"], true, "the most days");
    assert_eq!(ask_longer(&live, 1, 109_800)["reason"], "other");
    assert_eq!(ask_longer(&live, 1, u32::MAX)["reason"], "other");
}
