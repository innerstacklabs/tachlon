//! Checked add, remove and restore of a link, and the cycle path (gs#343).
//! The reference for each edit is a schedule built from scratch with the
//! dependency set the edit should leave in force.

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

use common::{assert_dates, at, check_invariants, cont_cal, dep, task, want};
use engine::LinkType::{FF, FS, SS};
use engine::{DepId, Dependency, Rng, Schedule, ScheduleError, TaskId, generate};

fn link(p: u32, s: u32, kind: engine::LinkType) -> Dependency {
    Dependency {
        pred: TaskId(p),
        succ: TaskId(s),
        kind,
        lag: 0,
    }
}

/// The same tasks and calendars with only the live dependencies, added unchecked.
fn from_scratch(s: &Schedule) -> Schedule {
    let mut c = Schedule::new();
    for cal in s.calendars() {
        c.add_calendar(cal.clone());
    }
    c.set_lag_calendar(s.lag_calendar())
        .expect("the lag calendar is accepted");
    for t in s.tasks() {
        c.add_task(t.clone()).expect("the task is valid");
    }
    for (_, d) in s.live_dependencies() {
        c.add_dependency(*d).expect("the link is valid");
    }
    c
}

fn assert_like_scratch(s: &mut Schedule, label: &str) {
    let got = s.compute().unwrap_or_else(|e| panic!("{label}: {e}"));
    let want = from_scratch(s).compute().expect("the schedule computes");
    assert_eq!(got, want, "{label}");
}

/// A chain 0 -> 1 -> 2 -> 3 of four one-day tasks, plus a loose task 4.
fn chain() -> Schedule {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let ids: Vec<TaskId> = (0..5).map(|_| task(&mut s, cal, 480, None)).collect();
    for w in ids[..4].windows(2) {
        dep(&mut s, w[0], w[1], FS, 0);
    }
    s
}

/// Everything an edit that is refused must leave alone.
fn state(s: &Schedule) -> (Vec<Dependency>, Vec<bool>, usize, bool) {
    let live: Vec<bool> = (0..s.dependencies().len())
        .map(|i| s.is_dependency_live(DepId(i as u32)))
        .collect();
    (
        s.dependencies().to_vec(),
        live,
        s.live_task_count(),
        s.has_cached_order(),
    )
}

#[test]
fn link_cycle_agrees_with_unchecked_add_and_compute() {
    let mut rng = Rng::new(7);
    let mut loops = 0;
    for seed in 0..5u64 {
        let mut s = generate(seed, 300);
        s.compute().unwrap();
        for _ in 0..100 {
            let p = TaskId(rng.range(0, 299) as u32);
            let q = TaskId(rng.range(0, 299) as u32);
            if p == q {
                continue;
            }
            let found = s.link_cycle(p, q).unwrap();
            let mut twin = s.clone();
            twin.add_dependency(Dependency {
                pred: p,
                succ: q,
                kind: FS,
                lag: 0,
            })
            .unwrap();
            let cycles = matches!(twin.compute(), Err(ScheduleError::Cycle(_)));
            assert_eq!(found.is_some(), cycles, "seed {seed}: {p} -> {q}");
            if let Some(path) = found {
                loops += 1;
                assert_eq!(path[..2], [p, q], "the path starts with the new link");
                for (i, &a) in path.iter().enumerate() {
                    let b = path[(i + 1) % path.len()];
                    let linked = (a == p && b == q)
                        || s.live_dependencies()
                            .any(|(_, d)| d.pred == a && d.succ == b);
                    assert!(linked, "{a} is not a predecessor of {b} in {path:?}");
                }
            }
        }
        assert!(s.has_cached_order(), "the check keeps the cached graph");
    }
    assert!(loops >= 10, "the sample reaches loops ({loops})");
}

#[test]
fn the_path_names_the_loop() {
    let mut s = chain();
    // 3 -> 0 closes 0 -> 1 -> 2 -> 3 -> 0.
    let path = s.link_cycle(TaskId(3), TaskId(0)).unwrap().unwrap();
    assert_eq!(path, [3, 0, 1, 2].map(TaskId));
    assert_eq!(s.add_link(link(3, 0, FS)), Err(ScheduleError::Cycle(path)));
    assert_eq!(s.link_cycle(TaskId(0), TaskId(3)).unwrap(), None);
    assert_eq!(s.link_cycle(TaskId(4), TaskId(0)).unwrap(), None);
}

#[test]
fn add_remove_and_restore_solve_like_a_schedule_built_from_scratch() {
    for seed in 0..10u64 {
        let mut s = generate(seed, 200);
        let mut rng = Rng::new(seed ^ 0x11);
        let mut added = Vec::new();
        while added.len() < 12 {
            let (p, q) = (rng.range(0, 199) as u32, rng.range(0, 199) as u32);
            let kind = [FS, SS, FF][rng.range(0, 2) as usize];
            let lag = rng.range(-1, 3) * 480;
            if let Ok(id) = s.add_link(Dependency {
                lag,
                ..link(p, q, kind)
            }) {
                added.push(id);
                assert_like_scratch(&mut s, &format!("seed {seed} add {id}"));
            }
        }
        let original = s.compute().unwrap();
        for &id in &added {
            s.remove_dependency(id).unwrap();
            assert!(!s.is_dependency_live(id));
            assert_like_scratch(&mut s, &format!("seed {seed} remove {id}"));
        }
        for &id in &added {
            s.restore_dependency(id).unwrap();
            assert!(s.is_dependency_live(id));
            assert_like_scratch(&mut s, &format!("seed {seed} restore {id}"));
        }
        assert_eq!(s.live_dependencies().count(), s.dependencies().len());
        s.remove_dependency(added[0]).unwrap();
        s.restore_dependency(added[0]).unwrap();
        assert_ne!(s.compute().unwrap().dates.len(), 0);
        let _ = original;
    }
}

#[test]
fn a_removed_link_keeps_its_id_and_a_re_add_takes_a_new_slot() {
    let mut s = chain();
    let gone = DepId(1);
    s.remove_dependency(gone).unwrap();
    assert_eq!(s.dependencies().len(), 3);
    assert_eq!(s.live_dependencies().count(), 2);
    let again = s.add_link(link(1, 2, FS)).unwrap();
    assert_eq!(again, DepId(3));
    assert_eq!(
        s.restore_dependency(gone),
        Err(ScheduleError::DuplicateDependency(again))
    );
    assert!(!s.is_dependency_live(gone));
}

#[test]
fn restoring_can_close_a_loop_and_is_refused() {
    let mut s = chain();
    s.remove_dependency(DepId(0)).unwrap(); // 0 -> 1 gone
    s.add_link(link(1, 0, FS)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert!(matches!(
        s.restore_dependency(DepId(0)),
        Err(ScheduleError::Cycle(_))
    ));
    assert_eq!(state(&s), before);
}

#[test]
fn a_removed_link_with_a_deleted_end_stays_removed_after_restore_task() {
    let mut s = chain();
    s.remove_dependency(DepId(1)).unwrap();
    s.delete_task(TaskId(2)).unwrap();
    s.restore_task(TaskId(2)).unwrap();
    assert!(!s.is_dependency_live(DepId(1)));
    assert_like_scratch(&mut s, "after restore_task");
    // The link is still removed, so the pair is free to be linked again.
    s.add_link(link(1, 2, SS)).unwrap();
    // Removing a link whose end is deleted works, but restoring it is refused
    // until the task is back.
    s.delete_task(TaskId(3)).unwrap();
    s.remove_dependency(DepId(2)).unwrap();
    let before = state(&s);
    assert_eq!(
        s.restore_dependency(DepId(2)),
        Err(ScheduleError::UnknownTask(TaskId(3)))
    );
    assert_eq!(state(&s), before);
    assert!(!s.is_dependency_live(DepId(2)));
    s.restore_task(TaskId(3)).unwrap();
    assert!(!s.is_dependency_live(DepId(2)));
    s.restore_dependency(DepId(2)).unwrap();
    assert!(s.is_dependency_live(DepId(2)));
    assert_like_scratch(&mut s, "end restored");
}

#[test]
fn restoring_a_link_with_a_deleted_end_is_refused_for_a_duplicate() {
    let mut s = chain();
    s.remove_dependency(DepId(0)).unwrap(); // 0 -> 1 gone
    let again = s.add_link(link(0, 1, FS)).unwrap();
    s.delete_task(TaskId(1)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert_eq!(
        s.restore_dependency(DepId(0)),
        Err(ScheduleError::UnknownTask(TaskId(1)))
    );
    assert_eq!(state(&s), before);
    // Task 1 back: the old link is now a checked duplicate of the new one.
    s.restore_task(TaskId(1)).unwrap();
    assert_eq!(
        s.restore_dependency(DepId(0)),
        Err(ScheduleError::DuplicateDependency(again))
    );
    assert!(!s.is_dependency_live(DepId(0)));
    assert_like_scratch(&mut s, "no duplicate");
}

#[test]
fn restoring_a_link_with_a_deleted_end_is_refused_for_a_loop() {
    let mut s = chain();
    s.remove_dependency(DepId(0)).unwrap(); // 0 -> 1 gone
    s.add_link(link(3, 0, FS)).unwrap();
    s.delete_task(TaskId(0)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert_eq!(
        s.restore_dependency(DepId(0)),
        Err(ScheduleError::UnknownTask(TaskId(0)))
    );
    assert_eq!(state(&s), before);
    s.restore_task(TaskId(0)).unwrap();
    assert!(matches!(
        s.restore_dependency(DepId(0)),
        Err(ScheduleError::Cycle(_))
    ));
    assert!(!s.is_dependency_live(DepId(0)));
    assert!(s.compute().is_ok(), "no loop reached the schedule");
    assert_like_scratch(&mut s, "no loop");
}

#[test]
fn refusals_change_nothing() {
    let mut s = chain();
    s.delete_task(TaskId(4)).unwrap();
    s.compute().unwrap();
    s.remove_dependency(DepId(2)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert!(before.3, "the graph is cached");
    let refused = [
        (link(0, 1, SS), ScheduleError::DuplicateDependency(DepId(0))),
        (link(0, 0, FS), ScheduleError::SelfDependency(TaskId(0))),
        (link(0, 9, FS), ScheduleError::UnknownTask(TaskId(9))),
        (link(4, 0, FS), ScheduleError::UnknownTask(TaskId(4))),
        (link(0, 4, FS), ScheduleError::UnknownTask(TaskId(4))),
    ];
    for (dep, want) in refused {
        assert_eq!(s.add_link(dep), Err(want), "{dep:?}");
        assert_eq!(state(&s), before, "{dep:?}");
    }
    let loop_back = s.add_link(link(1, 0, FS));
    assert!(matches!(loop_back, Err(ScheduleError::Cycle(_))));
    assert_eq!(state(&s), before);
    // The other direction between two linked tasks is no duplicate, but a loop.
    assert_eq!(
        s.remove_dependency(DepId(2)),
        Err(ScheduleError::UnknownDependency(DepId(2)))
    );
    assert_eq!(
        s.remove_dependency(DepId(40)),
        Err(ScheduleError::UnknownDependency(DepId(40)))
    );
    assert_eq!(
        s.restore_dependency(DepId(40)),
        Err(ScheduleError::UnknownDependency(DepId(40)))
    );
    assert_eq!(state(&s), before);
    assert_eq!(s.restore_dependency(DepId(0)), Ok(()), "a live link stays");
    assert_eq!(state(&s), before);
}

#[test]
fn an_importer_can_still_add_a_loop_unchecked() {
    let mut s = chain();
    s.add_dependency(link(3, 0, FS)).unwrap();
    s.add_dependency(link(0, 1, FS)).unwrap(); // a duplicate
    assert!(matches!(s.compute(), Err(ScheduleError::Cycle(_))));
    assert!(matches!(
        s.link_cycle(TaskId(0), TaskId(4)),
        Err(ScheduleError::Cycle(_))
    ));
}

#[test]
fn driving_readers_skip_a_removed_link_with_and_without_the_graph() {
    let mut s = chain();
    let sol = s.compute().unwrap();
    assert_eq!(s.driving_predecessors(&sol, TaskId(1)), [TaskId(0)]);
    assert_eq!(s.driven_successors(&sol, TaskId(0)), [TaskId(1)]);
    s.remove_dependency(DepId(0)).unwrap();
    assert!(!s.has_cached_order(), "the no-graph fallback is the reader");
    // The solution is stale on purpose: 0 still ends where 1 starts.
    assert_eq!(s.driving_predecessors(&sol, TaskId(1)), Vec::new());
    assert_eq!(s.driven_successors(&sol, TaskId(0)), Vec::new());
    let sol = s.compute().unwrap();
    assert_eq!(s.driving_predecessors(&sol, TaskId(1)), Vec::new());
    assert!(!s.longest_path(&sol).contains(&TaskId(0)));
    s.restore_dependency(DepId(0)).unwrap();
    let sol = s.compute().unwrap();
    assert_eq!(s.driving_predecessors(&sol, TaskId(1)), [TaskId(0)]);
}

#[test]
fn restore_task_refuses_links_that_close_a_loop() {
    let mut s = chain();
    s.delete_task(TaskId(1)).unwrap();
    s.add_link(link(2, 0, FS)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert!(matches!(
        s.restore_task(TaskId(1)),
        Err(ScheduleError::Cycle(_))
    ));
    assert_eq!(state(&s), before);
    assert!(s.is_deleted(TaskId(1)));
    assert_like_scratch(&mut s, "loop refused");
}

#[test]
fn restore_task_refuses_a_link_that_repeats_a_live_one() {
    let mut s = chain();
    // The unchecked importer door can leave a second 0 -> 1 beside the first.
    let again = s.add_dependency(link(0, 1, FS)).unwrap();
    s.delete_task(TaskId(1)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert!(matches!(
        s.restore_task(TaskId(1)),
        Err(ScheduleError::DuplicateDependency(d)) if d == DepId(0) && again == DepId(3)
    ));
    assert_eq!(state(&s), before);
    assert!(s.is_deleted(TaskId(1)));
    assert_like_scratch(&mut s, "duplicate refused");
}

#[test]
fn set_dependency_re_solves_like_a_fresh_schedule() {
    // Hand-solved: four 8-hour tasks on a 24/7 calendar, from the origin. Link 0 -> 1 becomes
    // SS+2h: task 1 starts 120 minutes after task 0 starts, and the tasks after it follow.
    let mut s = chain();
    let before = s.compute().unwrap();
    assert_eq!(before.dates[1].es, 480);
    s.set_dependency(DepId(0), SS, 120).unwrap();
    let after = s.compute().unwrap();
    let starts: Vec<i64> = after.dates.iter().map(|d| d.es).collect();
    assert_eq!(starts, vec![0, 120, 600, 1080, 0]);
    assert_eq!(after.dates[3].ef, 1560);
    assert_like_scratch(&mut s, "hand-solved case");

    // A seeded set of random changes: each solve equals the schedule built with the link as set.
    for seed in 0..10u64 {
        let mut s = generate(seed, 200);
        let mut rng = Rng::new(seed ^ 0x77);
        let mut changed = 0;
        while changed < 12 {
            let live: Vec<DepId> = s.live_dependencies().map(|(id, _)| id).collect();
            let id = live[rng.range(0, live.len() as i64 - 1) as usize];
            let kind = [FS, SS, FF, engine::LinkType::SF][rng.range(0, 3) as usize];
            let lag = rng.range(-2, 3) * 480;
            if s.set_dependency(id, kind, lag).is_ok() {
                changed += 1;
                assert_eq!(s.dependencies()[id.0 as usize].kind, kind);
                assert_eq!(s.dependencies()[id.0 as usize].lag, lag);
                assert_like_scratch(&mut s, &format!("seed {seed} set {id}"));
            }
        }
    }
}

#[test]
fn set_dependency_refuses_like_the_add_path_and_changes_nothing() {
    let mut s = chain();
    s.compute().unwrap();
    s.remove_dependency(DepId(1)).unwrap();
    let before = state(&s);
    // A removed and an unknown id.
    for dep in [DepId(1), DepId(99)] {
        assert_eq!(
            s.set_dependency(dep, SS, 60),
            Err(ScheduleError::UnknownDependency(dep))
        );
    }
    assert_eq!(state(&s), before);

    // An import can hold two links on a pair; the second may not become the first's twin.
    let mut s = chain();
    let twin = s.add_dependency(link(0, 1, SS)).unwrap();
    s.compute().unwrap();
    let before = state(&s);
    assert_eq!(
        s.set_dependency(twin, FS, 0),
        Err(ScheduleError::DuplicateDependency(DepId(0)))
    );
    assert_eq!(state(&s), before);
    // Another type on the pair is not a twin, and a lag change keeps its own type.
    s.set_dependency(twin, FF, 30).unwrap();
    s.set_dependency(twin, FF, 60).unwrap();

    // A loop already in an imported schedule is reported, as the add path reports it.
    let mut s = chain();
    s.add_dependency(link(3, 0, FS)).unwrap();
    let before = state(&s);
    assert!(matches!(
        s.set_dependency(DepId(0), SS, 0),
        Err(ScheduleError::Cycle(_))
    ));
    assert_eq!(state(&s), before);
}

#[test]
fn an_imported_twin_does_not_stop_a_lag_edit_and_replay_restores_the_type() {
    let mut s = chain();
    let twin = s.add_dependency(link(0, 1, FS)).unwrap();
    s.compute().unwrap();
    // The lag changes; the type stays: no new twin.
    s.set_dependency(twin, FS, 60).unwrap();
    assert_like_scratch(&mut s, "lag beside a twin");
    s.set_dependency(twin, SS, 30).unwrap();
    // A person's change back to FS would repeat link 0; history's replay is exact.
    assert_eq!(
        s.set_dependency(twin, FS, 60),
        Err(ScheduleError::DuplicateDependency(DepId(0)))
    );
    s.set_dependency_replay(twin, FS, 60).unwrap();
    assert_eq!(
        (
            s.dependencies()[twin.0 as usize].kind,
            s.dependencies()[twin.0 as usize].lag
        ),
        (FS, 60)
    );
    assert_like_scratch(&mut s, "replay beside a twin");
    s.remove_dependency(twin).unwrap();
    assert_eq!(
        s.set_dependency_replay(twin, SS, 0),
        Err(ScheduleError::UnknownDependency(twin))
    );
}

/// tl#1234: on Mon-Fri 08:00-16:00, an SF bound at 08:00 Tuesday is met by a finish at 16:00
/// Monday (no working minute between), as Bryntum reads it; not by 08:01 Tuesday. P (SNET
/// Tuesday) -SF-> S -FS-> T (SNET Tuesday), one day each: S is Monday, and the backward pass
/// lets P start Tuesday 08:00 against S's late finish of Monday 16:00.
#[test]
fn sf_finish_at_day_end_meets_start_of_next_day() {
    let mut week: [Vec<engine::DayPeriod>; 7] = Default::default();
    for day in &mut week[..5] {
        *day = vec![(480, 960)];
    }
    let mut s = Schedule::new();
    let cal = s.add_calendar(engine::Calendar::new(week, &[]).unwrap());
    let p = task(&mut s, cal, 480, Some(at(1, 8, 0)));
    let succ = task(&mut s, cal, 480, None);
    let t = task(&mut s, cal, 480, Some(at(1, 8, 0)));
    dep(&mut s, p, succ, engine::LinkType::SF, 0);
    dep(&mut s, succ, t, FS, 0);
    let sol = s.compute().unwrap();
    let day = |d| want(at(d, 8, 0), at(d, 16, 0), at(d, 8, 0), at(d, 16, 0), 0);
    let (mon, tue) = (day(0), day(1));
    assert_dates(&sol, succ, mon);
    assert_dates(&sol, p, tue);
    assert_dates(&sol, t, tue);
    check_invariants(&s, &sol);
}
