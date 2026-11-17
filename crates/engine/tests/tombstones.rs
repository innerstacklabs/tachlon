//! Tombstones (gs#301): a deleted task keeps its id and its slot, is not
//! solved, and every dependency touching it stops applying. The reference for
//! each check is the same schedule built without the dead tasks.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

mod common;

use common::{cont_cal, dep, fuzz, oracle, seeds, task};
use engine::LinkType::{FF, FS, SS};
use engine::{
    DAY, DepId, Dependency, LagCalendar, Rng, Schedule, ScheduleError, Solution, TaskDates, TaskId,
    generate,
};

const ZERO: TaskDates = TaskDates {
    es: 0,
    ef: 0,
    ls: 0,
    lf: 0,
    float: 0,
    finished: false,
};

/// Delete about `percent` of the tasks, chosen from `seed`.
fn delete_some(s: &mut Schedule, seed: u64, percent: i64) -> Vec<TaskId> {
    let mut rng = Rng::new(seed ^ 0x7d0e);
    let dead: Vec<TaskId> = (0..s.tasks().len() as u32)
        .map(TaskId)
        .filter(|_| rng.range(0, 99) < percent)
        .collect();
    for &t in &dead {
        s.delete_task(t).expect("the task exists");
    }
    dead
}

/// The schedule without its dead tasks and their dependencies, with ids
/// renumbered; `map[old]` is the new id of a live task.
fn compact(s: &Schedule) -> (Schedule, Vec<Option<TaskId>>) {
    let mut c = Schedule::new();
    for cal in s.calendars() {
        c.add_calendar(cal.clone());
    }
    c.set_lag_calendar(s.lag_calendar())
        .expect("the lag calendar is accepted");
    let map: Vec<Option<TaskId>> = s
        .tasks()
        .iter()
        .map(|t| (!t.deleted).then(|| c.add_task(t.clone()).expect("the task is valid")))
        .collect();
    for (k, d) in s.dependencies().iter().enumerate() {
        if let (Some(p), Some(q)) = (map[d.pred.0 as usize], map[d.succ.0 as usize]) {
            let new = c
                .add_dependency(Dependency {
                    pred: p,
                    succ: q,
                    ..*d
                })
                .expect("the link is valid");
            let own = s.dependency_lag_calendar(DepId(k as u32));
            c.set_dependency_lag_calendar(new, own)
                .expect("the lag calendar is accepted");
        }
    }
    (c, map)
}

/// Every live task's dates equal its compacted twin's; every dead slot is zero.
fn assert_equivalent(s: &mut Schedule, label: &str) -> Solution {
    let sol = s.compute().unwrap_or_else(|e| panic!("{label}: {e}"));
    let (mut c, map) = compact(s);
    let want = c.compute().expect("the schedule computes");
    assert_eq!(sol.project_finish, want.project_finish, "{label}");
    assert_eq!(sol.dates.len(), s.tasks().len(), "{label}: slot count");
    for (i, m) in map.iter().enumerate() {
        match m {
            Some(n) => assert_eq!(sol.dates[i], want.dates[n.0 as usize], "{label}: task {i}"),
            None => assert_eq!(sol.dates[i], ZERO, "{label}: dead slot {i}"),
        }
    }
    sol
}

#[test]
fn compacted_equivalence_over_seeds() {
    for seed in 0..20 {
        let mut s = generate(seed, 600);
        let dead = delete_some(&mut s, seed, 15);
        assert_ne!(dead, Vec::new());
        assert_equivalent(&mut s, &format!("seed {seed}"));
        assert_eq!(s.live_task_count(), 600 - dead.len());
    }
}

#[test]
fn cone_equals_full_with_tombstones() {
    for seed in 0..10 {
        let mut s = generate(seed, 400);
        delete_some(&mut s, seed, 15);
        let before = s.compute().unwrap();
        for t in (0..400).step_by(23).map(TaskId) {
            if s.is_deleted(t) {
                continue;
            }
            for shift in [6 * DAY, -3 * DAY] {
                let mut edited = s.clone();
                edited
                    .set_snet(t, Some((before.get(t).es + shift).max(0)))
                    .unwrap();
                let want = edited.compute().unwrap();
                let mut sol = before.clone();
                edited.recompute_cone(&mut sol, t).unwrap();
                assert_eq!(sol, want, "seed {seed}, {t}, shift {shift}");
            }
        }
    }
}

#[test]
fn a_deleted_cone_root_is_unknown() {
    let mut s = generate(1, 50);
    let mut sol = s.compute().unwrap();
    s.delete_task(TaskId(7)).unwrap();
    let kept = sol.clone();
    assert_eq!(
        s.recompute_cone(&mut sol, TaskId(7)),
        Err(ScheduleError::UnknownTask(TaskId(7)))
    );
    assert_eq!(sol, kept);
}

#[test]
fn brute_force_oracle_agrees_on_the_live_subset() {
    for seed in seeds(300) {
        let mut s = fuzz::schedule(seed);
        if delete_some(&mut s, seed, 30).is_empty() {
            continue;
        }
        let (mut c, _) = compact(&s);
        if c.tasks().is_empty() {
            continue;
        }
        oracle::assert_matches(&mut c, &format!("seed {seed}"));
        assert_equivalent(&mut s, &format!("seed {seed}"));
    }
}

#[test]
fn every_lag_calendar_keeps_the_equivalence() {
    for (k, lag) in [
        LagCalendar::Successor,
        LagCalendar::Predecessor,
        LagCalendar::TwentyFourHour,
        LagCalendar::Fixed(engine::CalendarId(1)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut s = generate(100 + k as u64, 300);
        s.set_lag_calendar(lag).unwrap();
        delete_some(&mut s, k as u64, 20);
        assert_equivalent(&mut s, &format!("{lag:?}"));
    }
}

#[test]
fn delete_then_restore_gives_an_identical_solution() {
    for seed in 0..10 {
        let mut s = generate(seed, 400);
        let whole = s.compute().unwrap();
        let tasks = s.tasks().to_vec();
        let deps = s.dependencies().to_vec();
        let dead = delete_some(&mut s, seed, 20);
        s.compute().unwrap();
        for &t in dead.iter().rev() {
            s.restore_task(t).unwrap();
        }
        assert_eq!(s.compute().unwrap(), whole, "seed {seed}");
        assert_eq!(s.tasks(), tasks.as_slice());
        assert_eq!(s.dependencies(), deps.as_slice());
    }
}

#[test]
fn delete_restore_and_the_flags() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let (a, b) = (task(&mut s, c, 60, None), task(&mut s, c, 60, None));
    assert!(!s.is_deleted(a) && !s.is_deleted(TaskId(9)));
    s.delete_task(a).unwrap();
    assert!(s.is_deleted(a));
    assert_eq!((s.live_task_count(), s.tasks().len()), (1, 2));
    assert_eq!(s.delete_task(a), Err(ScheduleError::UnknownTask(a)));
    assert_eq!(
        s.delete_task(TaskId(9)),
        Err(ScheduleError::UnknownTask(TaskId(9)))
    );
    s.restore_task(a).unwrap();
    s.restore_task(a).unwrap();
    assert_eq!(
        s.restore_task(TaskId(9)),
        Err(ScheduleError::UnknownTask(TaskId(9)))
    );
    assert_eq!(s.live_task_count(), 2);
    dep(&mut s, a, b, FS, 0);
    s.delete_task(b).unwrap();
    assert!(!s.is_dependency_live(engine::DepId(0)));
    s.restore_task(b).unwrap();
    assert!(s.is_dependency_live(engine::DepId(0)));
}

#[test]
fn a_cycle_through_a_dead_task_is_not_a_cycle() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let t: Vec<TaskId> = (0..4).map(|_| task(&mut s, c, 60, None)).collect();
    dep(&mut s, t[0], t[1], FS, 0);
    dep(&mut s, t[1], t[2], FS, 0);
    dep(&mut s, t[2], t[0], FS, 0);
    dep(&mut s, t[2], t[3], FS, 0);
    assert!(matches!(s.compute(), Err(ScheduleError::Cycle(_))));
    s.delete_task(t[1]).unwrap();
    let sol = s.compute().unwrap();
    assert_eq!(sol.get(t[1]), &ZERO);
    assert_eq!(sol.get(t[3]).es, sol.get(t[2]).ef);
    // Restoring it would close the loop: refused, and the schedule stays acyclic.
    assert!(matches!(s.restore_task(t[1]), Err(ScheduleError::Cycle(_))));
    assert!(s.is_deleted(t[1]));
    assert!(s.compute().is_ok());
}

#[test]
fn a_live_cycle_is_reported_without_dead_ids() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let t: Vec<TaskId> = (0..7).map(|_| task(&mut s, c, 60, None)).collect();
    dep(&mut s, t[0], t[1], FS, 0); // dead tail into the cycle
    dep(&mut s, t[1], t[2], SS, 0);
    dep(&mut s, t[2], t[3], FS, 0);
    dep(&mut s, t[3], t[1], FF, 0);
    dep(&mut s, t[3], t[4], FS, 0);
    dep(&mut s, t[4], t[5], FS, 0); // dead after it
    dep(&mut s, t[5], t[6], FS, 0);
    dep(&mut s, t[6], t[2], FS, 0); // a second cycle, through dead tasks
    for d in [0, 4, 5] {
        s.delete_task(t[d]).unwrap();
    }
    match s.compute() {
        Err(ScheduleError::Cycle(mut cycle)) => {
            cycle.sort();
            assert_eq!(cycle, vec![t[1], t[2], t[3]]);
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn longest_path_ignores_a_dead_task_whose_slot_is_zero() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    // Every live task is a milestone at the origin, so the project finish is 0,
    // the value a dead slot reads.
    let a = task(&mut s, c, 0, None);
    let dead = task(&mut s, c, 60, None);
    let b = task(&mut s, c, 0, None);
    dep(&mut s, a, b, FS, 0);
    s.delete_task(dead).unwrap();
    let sol = s.compute().unwrap();
    assert_eq!(sol.project_finish, 0);
    assert_eq!(s.longest_path(&sol), vec![a, b]);
}

#[test]
fn driving_queries_skip_dead_links_with_and_without_the_graph() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let t: Vec<TaskId> = (0..3).map(|_| task(&mut s, c, 60, None)).collect();
    dep(&mut s, t[0], t[1], FS, 0);
    dep(&mut s, t[1], t[2], FS, 0);
    let sol = s.compute().unwrap();
    assert_eq!(s.driving_predecessors(&sol, t[2]), vec![t[1]]);
    s.delete_task(t[1]).unwrap(); // clears the cached graph: the fallback path
    assert!(!s.has_cached_order());
    assert_eq!(s.driving_predecessors(&sol, t[2]), Vec::new());
    assert_eq!(s.driven_successors(&sol, t[0]), Vec::new());
    let sol = s.compute().unwrap();
    assert_eq!(s.driving_predecessors(&sol, t[2]), Vec::new());
    assert_eq!(s.driving_predecessors(&sol, t[1]), Vec::new());
    assert_eq!(s.driven_successors(&sol, t[1]), Vec::new());
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "a and b are the two tasks of the pair, tried in both directions"
)]
fn every_mutator_refuses_a_deleted_task_and_changes_nothing() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let (a, b) = (task(&mut s, c, 60, None), task(&mut s, c, 120, Some(30)));
    dep(&mut s, a, b, FS, 0);
    s.compute().unwrap();
    s.delete_task(b).unwrap();
    s.compute().unwrap();
    let (tasks, deps) = (s.tasks().to_vec(), s.dependencies().to_vec());
    let unknown = Err(ScheduleError::UnknownTask(b));
    assert_eq!(s.set_snet(b, Some(5)), unknown);
    assert_eq!(s.set_duration(b, 5), unknown);
    for (pred, succ) in [(a, b), (b, a)] {
        let d = Dependency {
            pred,
            succ,
            kind: FS,
            lag: 0,
        };
        assert_eq!(s.add_dependency(d), Err(ScheduleError::UnknownTask(b)));
    }
    assert_eq!((s.tasks(), s.dependencies()), (&tasks[..], &deps[..]));
    assert!(s.has_cached_order(), "a refusal keeps the cached order");
}
