//! Native engine benches (plan: criterion at 10k/50k/100k).
//!
//! * `compute/<n>`: forward + backward pass with the topological order cached
//!   (the per-drag cost in D6).
//! * `compute_cold/<n>`: the same plus Kahn and the adjacency build.
//! * `stress_drag/<n>`: move the stress fixture's head task by a day, then
//!   recompute (the M3 drag on a schedule the head reaches almost entirely).
//! * `stress_drag_cone/<n>`: the same drag, updating the solution with
//!   `recompute_cone` instead of a full `compute`.
//! * `stress_drag_cone_exceptions/<n>`: the same as `stress_drag_cone`, on
//!   `stress_with_exceptions`: the same fixture with about ten working
//!   exceptions a year (working Saturdays, shortened days) on calendars 0
//!   and 3. Compare it with `stress_drag_cone`.
//! * `drag_step_full/<n>` and `drag_step_cone/<n>`: one step of a bar drag
//!   on the generated schedule, as the browser profile drags it (a critical
//!   task and four tasks spread over the start order, 40 half-day steps
//!   each), with a full `compute` and with `recompute_cone`. Same schedule,
//!   same steps, so the two numbers compare.
//! * `drag_finish_full/<n>` and `drag_finish_cone/<n>`: toggle the first
//!   critical task 20 days later and back, moving the project finish on every
//!   iteration, with a full `compute` and with `recompute_cone`.
//! * `drag_step_cone_predecessor/<n>` and `drag_finish_cone_predecessor/<n>`:
//!   the two cone groups above with the lag counted in the predecessor's
//!   calendar (`LagCalendar::Predecessor`), against the `Successor` default.
//! * `duration_full/<n>`: grow a pinned task by 60 working minutes and back on
//!   `stress(20261020, n)`, finish fixed, full `compute` (`cpm/duration.rs`).
//! * `duration_cone/<n>`: the same edit through
//!   `recompute_duration_cone_moved`, as `Edit::SetDuration` runs it.
//! * `duration_finish`, `duration_large` and `duration_backward`: the three
//!   edits of `cpm/duration_groups.rs`, each with a `cone` and a `full` run.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

#[path = "cpm/duration.rs"]
mod duration;
#[path = "cpm/duration_groups.rs"]
mod duration_groups;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use engine::{
    DAY, DurationCone, LagCalendar, Schedule, Solution, TaskId, generate, stress,
    stress_with_exceptions,
};

/// A fixture builder: `stress` or `stress_with_exceptions`.
type Fixture = fn(u64, usize) -> (Schedule, TaskId);

const SIZES: [usize; 3] = [10_000, 50_000, 100_000];
const SEED: u64 = 42;
/// Drag steps per target before the next target is picked.
const STEPS: usize = 40;

/// The drag targets of the browser profile: the first critical task in start
/// order, and the tasks at start-order rows 200, 12000, 25000 and 40000 of a
/// 50 000-task schedule (scaled to `n`).
fn drag_targets(sol: &Solution) -> Vec<TaskId> {
    let n = sol.dates.len();
    let mut by_start: Vec<u32> = (0..n as u32).collect();
    by_start.sort_by_key(|&t| (sol.dates[t as usize].es, t));
    let critical = by_start
        .iter()
        .copied()
        .find(|&t| sol.dates[t as usize].is_critical())
        .expect("a critical task");
    let mut targets = vec![TaskId(critical)];
    for row in [200, 12_000, 25_000, 40_000] {
        targets.push(TaskId(by_start[row * n / 50_000]));
    }
    targets
}

/// The step `i` of the drag sequence: the target, the start it asks for, and
/// whether this step begins a new target (the previous one is released
/// first).
fn drag_step(i: usize, targets: &[TaskId], base_es: &[i64]) -> (TaskId, i64, Option<TaskId>) {
    let (sweep, k) = (i / STEPS, i % STEPS);
    let task = targets[sweep % targets.len()];
    let wanted = base_es[task.0 as usize] + (k as i64 + 1) * DAY / 2;
    let release = (k == 0 && sweep > 0).then(|| targets[(sweep - 1) % targets.len()]);
    (task, wanted, release)
}

/// One bench group of drag steps; `update` brings the solution up to date
/// after the constraint of a task changed.
fn drag_group(
    c: &mut Criterion,
    name: &str,
    lag: LagCalendar,
    update: fn(&mut Schedule, &mut Solution, TaskId),
) {
    let mut g = c.benchmark_group(name);
    g.sample_size(20);
    for n in SIZES {
        let mut s = generate(SEED, n);
        s.set_lag_calendar(lag).expect("a lag calendar");
        let mut sol = s.compute().expect("generated schedule computes");
        let targets = drag_targets(&sol);
        let base_es: Vec<i64> = sol.dates.iter().map(|d| d.es).collect();
        let base_snet: Vec<_> = targets
            .iter()
            .map(|t| s.tasks()[t.0 as usize].snet)
            .collect();
        let mut i = 0;
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let (task, wanted, release) = drag_step(i, &targets, &base_es);
                i += 1;
                if let Some(prev) = release {
                    let at = targets.iter().position(|&t| t == prev).expect("a target");
                    s.set_snet(prev, base_snet[at]).expect("task exists");
                    update(&mut s, &mut sol, prev);
                }
                s.set_snet(task, Some(wanted)).expect("task exists");
                update(&mut s, &mut sol, task);
            });
        });
    }
    g.finish();
}

/// One bench group of finish-moving drag steps; `update` brings the solution
/// up to date after the constraint of the first critical task changed.
fn drag_finish_group(
    c: &mut Criterion,
    name: &str,
    lag: LagCalendar,
    update: fn(&mut Schedule, &mut Solution, TaskId),
) {
    let mut g = c.benchmark_group(name);
    g.sample_size(20);
    for n in SIZES {
        let mut s = generate(SEED, n);
        s.set_lag_calendar(lag).expect("a lag calendar");
        let mut sol = s.compute().expect("generated schedule computes");
        let task = drag_targets(&sol)[0];
        let original_snet = s.tasks()[task.0 as usize].snet;
        let later = sol.dates[task.0 as usize].es + 20 * DAY;

        let original_finish = sol.project_finish;
        s.set_snet(task, Some(later)).expect("task exists");
        update(&mut s, &mut sol, task);
        assert_ne!(sol.project_finish, original_finish);
        s.set_snet(task, original_snet).expect("task exists");
        update(&mut s, &mut sol, task);

        let mut move_later = true;
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let snet = if move_later {
                    Some(later)
                } else {
                    original_snet
                };
                move_later = !move_later;
                s.set_snet(task, snet).expect("task exists");
                update(&mut s, &mut sol, task);
            });
        });
    }
    g.finish();
}

/// Alternate the pinned task's duration; both directions checked before timing.
fn duration_full(c: &mut Criterion) {
    let mut g = c.benchmark_group("duration_full");
    g.sample_size(20);
    for n in SIZES {
        let mut w = duration::Workload::new(n);
        for longer in [true, false] {
            w.step(longer);
            assert_eq!(w.solution, w.schedule.compute().expect("computes"));
            assert_ne!(w.moved, Vec::new());
        }
        let mut longer = true;
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                w.step(longer);
                longer = !longer;
            });
        });
    }
    g.finish();
}

/// The same edit as `duration_full`, through `recompute_duration_cone_moved`.
/// Both directions are checked before timing: the solution equals a full
/// compute of the edited schedule, and `moved` lists the tasks whose early
/// dates differ. The list is non-empty and no longer than `cap` (a tenth of
/// the tasks, at most 2048), and the call reports `DurationCone::Cone`, so the
/// timed call is the cone path and not a fallback to a full compute. The
/// engine's `cpm/duration/tests.rs` covers when a fallback happens.
fn duration_cone(c: &mut Criterion) {
    let mut g = c.benchmark_group("duration_cone");
    g.sample_size(20);
    for n in SIZES {
        let mut w = duration::Workload::new(n);
        let cap = (n / 10).clamp(1, 2048);
        for longer in [true, false] {
            let before = w.solution.clone();
            assert_eq!(
                w.step_cone(longer),
                DurationCone::Cone,
                "duration_cone/{n}: longer={longer}"
            );
            let full = w.schedule.clone().compute().expect("computes");
            assert_eq!(w.solution, full);
            let mut want = Vec::new();
            duration::moved_between(&before, &full, &mut want);
            let mut got = w.moved.clone();
            got.sort_by_key(|m| m.task.0);
            assert_eq!(got, want);
            assert!(!want.is_empty() && want.len() <= cap);
        }
        let mut longer = true;
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                w.step_cone(longer);
                longer = !longer;
            });
        });
    }
    g.finish();
}

#[expect(
    clippy::too_many_lines,
    reason = "one function registers every group of the bench, in order"
)]
fn benches(c: &mut Criterion) {
    let mut g = c.benchmark_group("compute");
    g.sample_size(20);
    for n in SIZES {
        let mut s = generate(SEED, n);
        s.compute().expect("generated schedule computes");
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| s.compute().expect("computes"));
        });
    }
    g.finish();

    let mut g = c.benchmark_group("compute_cold");
    g.sample_size(20);
    for n in SIZES {
        let mut s = generate(SEED, n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                s.clear_cached_order();
                s.compute().expect("computes")
            });
        });
    }
    g.finish();

    let mut g = c.benchmark_group("stress_drag");
    g.sample_size(20);
    for n in SIZES {
        let (mut s, head) = stress(SEED, n);
        s.compute().expect("stress fixture computes");
        let mut day = 0;
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                day = (day + 1) % 30;
                s.set_snet(head, Some(day * DAY)).expect("head exists");
                s.compute().expect("computes")
            });
        });
    }
    g.finish();

    let fixtures: [(&str, Fixture); 2] = [
        ("stress_drag_cone", stress),
        ("stress_drag_cone_exceptions", stress_with_exceptions),
    ];
    for (name, fixture) in fixtures {
        let mut g = c.benchmark_group(name);
        g.sample_size(20);
        for n in SIZES {
            let (mut s, head) = fixture(SEED, n);
            let mut sol = s.compute().expect("stress fixture computes");
            let mut day = 0;
            g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| {
                    day = (day + 1) % 30;
                    s.set_snet(head, Some(day * DAY)).expect("head exists");
                    s.recompute_cone(&mut sol, head).expect("computes");
                });
            });
        }
        g.finish();
    }

    // What the old drag did: a full compute, and the solution replaced.
    let default = LagCalendar::Successor;
    drag_group(c, "drag_step_full", default, |s, sol, _| {
        *sol = s.compute().expect("computes");
    });
    drag_group(c, "drag_step_cone", default, |s, sol, task| {
        s.recompute_cone(sol, task).expect("computes");
    });
    drag_finish_group(c, "drag_finish_full", default, |s, sol, _| {
        *sol = s.compute().expect("computes");
    });
    drag_finish_group(c, "drag_finish_cone", default, |s, sol, task| {
        s.recompute_cone(sol, task).expect("computes");
    });
    let predecessor = LagCalendar::Predecessor;
    drag_group(
        c,
        "drag_step_cone_predecessor",
        predecessor,
        |s, sol, task| s.recompute_cone(sol, task).expect("computes"),
    );
    drag_finish_group(
        c,
        "drag_finish_cone_predecessor",
        predecessor,
        |s, sol, task| s.recompute_cone(sol, task).expect("computes"),
    );
}

criterion_group!(
    cpm,
    benches,
    duration_full,
    duration_cone,
    duration_groups::groups
);
criterion_main!(cpm);
