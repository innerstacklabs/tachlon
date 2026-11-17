//! Property tests over generated schedules (any link type): every forward
//! and backward inequality holds, es <= ls, float >= 0, and the project
//! finish has a zero-float task ending at it (`common::check_invariants`).
//! Also the generator's own promises: determinism, density, the stress head.
//! And `Calendar::add` is monotone in `t` on calendars with exception days,
//! which the search in `cpm.rs` rests on.

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

use common::{check_invariants, seeds};
use engine::{Calendar, DayPeriod, Rng, TaskId, WEEK, generate, stress};

#[test]
fn generated_schedules_hold_every_invariant() {
    for seed in seeds(20) {
        let mut s = generate(seed, 1_000);
        let sol = s.compute().unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        check_invariants(&s, &sol);
    }
}

#[test]
fn longest_path_is_closed_under_driving_predecessors() {
    for seed in seeds(20) {
        let mut s = generate(seed, 1_000);
        let sol = s.compute().unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let path = s.longest_path(&sol);
        assert!(!path.is_empty(), "seed {seed}: empty path");
        assert!(
            path.windows(2).all(|pair| pair[0] < pair[1]),
            "seed {seed}: path is not strictly ascending"
        );

        let mut in_path = vec![false; s.tasks().len()];
        for &task in &path {
            in_path[task.0 as usize] = true;
        }
        for (i, dates) in sol.dates.iter().enumerate() {
            if dates.ef == sol.project_finish {
                assert!(in_path[i], "seed {seed}: missing finishing task {i}");
            }
        }

        let mut drives_member = vec![false; s.tasks().len()];
        for &task in &path {
            assert!(
                sol.get(task).ef <= sol.project_finish,
                "seed {seed}: {task} finishes after project finish"
            );
            for pred in s.driving_predecessors(&sol, task) {
                assert!(
                    in_path[pred.0 as usize],
                    "seed {seed}: missing driver {pred} of {task}"
                );
                drives_member[pred.0 as usize] = true;
            }
        }
        for &task in &path {
            let is_seed = sol.get(task).ef == sol.project_finish;
            assert!(
                is_seed || drives_member[task.0 as usize],
                "seed {seed}: stray path member {task}"
            );
        }
    }
}

#[test]
fn large_generated_schedules_hold_every_invariant() {
    for (seed, n) in [(1, 10_000), (2, 50_000)] {
        let mut s = generate(seed, n);
        let sol = s.compute().unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        check_invariants(&s, &sol);
    }
}

#[test]
fn stress_fixtures_hold_every_invariant() {
    for seed in seeds(5) {
        let (mut s, _) = stress(seed, 3_000);
        let sol = s.compute().unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        check_invariants(&s, &sol);
    }
}

#[test]
fn generator_is_deterministic_and_sized() {
    for n in [1_000, 10_000] {
        let (a, b) = (generate(7, n), generate(7, n));
        assert_eq!(a.tasks(), b.tasks());
        assert_eq!(a.dependencies(), b.dependencies());
        assert_eq!(a.tasks().len(), n);
        assert_ne!(
            generate(8, n).dependencies(),
            a.dependencies(),
            "seed matters"
        );
    }
    for n in [50_000, 100_000] {
        assert_eq!(generate(3, n).tasks().len(), n);
    }
}

#[test]
fn generator_shape() {
    let s = generate(11, 10_000);
    let per_task = s.dependencies().len() as f64 / s.tasks().len() as f64;
    assert!(
        (1.3..=1.7).contains(&per_task),
        "{per_task} dependencies per task"
    );
    let lags = s.dependencies().iter().filter(|d| d.lag != 0).count();
    assert!(lags > 1_000, "{lags} lags");
    let used: std::collections::HashSet<u32> = s.tasks().iter().map(|t| t.calendar.0).collect();
    assert_eq!(used.len(), 4, "a few calendars, all used");
}

/// The stress head reaches most of the schedule (the M3 drag target), and
/// moving it moves most early starts.
#[test]
fn stress_head_reaches_most_of_the_schedule() {
    let (mut s, head) = stress(5, 20_000);
    let n = s.tasks().len();
    let mut out: Vec<Vec<u32>> = vec![Vec::new(); n];
    for d in s.dependencies() {
        out[d.pred.0 as usize].push(d.succ.0);
    }
    let mut seen = vec![false; n];
    let mut stack = vec![head.0];
    seen[head.0 as usize] = true;
    while let Some(t) = stack.pop() {
        for &q in &out[t as usize] {
            if !seen[q as usize] {
                seen[q as usize] = true;
                stack.push(q);
            }
        }
    }
    let reached = seen.iter().filter(|&&r| r).count();
    assert!(reached * 10 >= n * 9, "head reaches {reached} of {n}");
    let per_task = s.dependencies().len() as f64 / n as f64;
    assert!(per_task > 2.5, "dense: {per_task} dependencies per task");

    let before = s.compute().unwrap();
    s.set_snet(head, Some(before.project_finish / 4)).unwrap();
    assert!(s.has_cached_order(), "a drag keeps the cached order");
    let after = s.compute().unwrap();
    check_invariants(&s, &after);
    let moved = (0..n)
        .filter(|&i| after.get(TaskId(i as u32)).es != before.get(TaskId(i as u32)).es)
        .count();
    assert!(
        moved * 2 >= n,
        "moving the head moved {moved} of {n} early starts"
    );
}

/// `add(t, w)` never decreases as `t` grows, for forward and backward `w`, on
/// random calendars with holidays, working weekend days and shortened or
/// lengthened weekdays, minute by minute over six weeks.
#[test]
fn add_is_monotone_on_calendars_with_exceptions() {
    for seed in seeds(12) {
        let mut rng = Rng::new(seed ^ 0x00ad_d0e0);
        let std: Vec<DayPeriod> = vec![(480, 720), (780, 1020)];
        let mut week: [Vec<DayPeriod>; 7] = Default::default();
        week[..5].fill(std);
        let mut exceptions: Vec<(i64, Vec<DayPeriod>)> = Vec::new();
        for day in 0..42 {
            if rng.chance(0.3) {
                let periods = match rng.range(0, 3) {
                    0 => vec![],
                    1 => vec![(rng.range(0, 600) as u16, rng.range(601, 1440) as u16)],
                    2 => vec![(0, 1440)],
                    _ => vec![(60, 120), (1000, 1100)],
                };
                exceptions.push((day, periods));
            }
        }
        let cal = Calendar::with_exceptions(week, &exceptions).unwrap();
        for w in [-1000, -61, -1, 1, 60, 541, 3000] {
            let mut prev = cal.add(0, w);
            for t in 1..6 * WEEK {
                let u = cal.add(t, w);
                assert!(u >= prev, "seed {seed} w {w}: add({t}) = {u} < {prev}");
                if w > 0 {
                    assert_eq!(cal.working_between(t, u), w, "seed {seed} w {w} t {t}");
                }
                prev = u;
            }
        }
    }
}

/// The working-exception stress variant keeps the stress fixture's tasks,
/// links and holidays, adds about ten working exceptions a year to calendars
/// 0 and 3 only, and still holds every invariant.
#[test]
fn stress_with_exceptions_changes_only_the_calendars() {
    let (mut a, head_a) = stress(20_261_020, 2_000);
    let (mut b, head_b) = engine::stress_with_exceptions(20_261_020, 2_000);
    assert_eq!(head_a, head_b);
    let key = |s: &engine::Schedule| {
        let tasks: Vec<_> = s
            .tasks()
            .iter()
            .map(|t| (t.duration, t.calendar, t.snet))
            .collect();
        let deps: Vec<_> = s
            .dependencies()
            .iter()
            .map(|d| (d.pred, d.succ, d.kind, d.lag))
            .collect();
        (tasks, deps)
    };
    assert!(key(&a) == key(&b), "same tasks and links");
    for (i, (ca, cb)) in a.calendars().iter().zip(b.calendars()).enumerate() {
        assert_eq!(ca.week(), cb.week());
        assert_eq!(ca.holidays(), cb.holidays(), "calendar {i} holidays");
        let working = cb.exceptions().len() - cb.holidays().len();
        if i == 0 || i == 3 {
            assert!((380..=400).contains(&working), "calendar {i}: {working}");
        } else {
            assert_eq!(working, 0, "calendar {i}");
        }
        assert_eq!(ca.exceptions().len(), ca.holidays().len());
    }
    let (sa, sb) = (a.compute().unwrap(), b.compute().unwrap());
    check_invariants(&b, &sb);
    assert!(
        sa.dates.iter().zip(&sb.dates).any(|(x, y)| x.es != y.es),
        "the exceptions move some dates"
    );
}
