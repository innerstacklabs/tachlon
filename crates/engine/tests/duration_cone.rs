//! `Schedule::recompute_duration_cone_moved` against `Schedule::compute`
//! (gs#58 PR 2): after any chain of duration edits the incremental result
//! equals a full compute, and `moved` is the early-date diff.
//!
//! Like `cone.rs`, the chain is carried forward by the incremental path alone.
//! The fuzz schedules cover calendars with holidays and working exceptions,
//! milestones, SNET, all four link types, negative lags and all four lag
//! calendars; the generated and stress schedules and the pinned PR 1 workload
//! cover size.
//!
//! The equivalence matrix (gs#520) is the second half of the file: the shuffled
//! eight-operation cycle of `common::duration_chain` (duration to zero, shorter,
//! plus one minute, plus 60 minutes, redrawn, kept, SNET set, SNET cleared) on the fuzz,
//! generated, stress and stress-with-exceptions schedules under the `Successor`,
//! `Predecessor` and `TwentyFourHour` lag calendars; a schedule with a tenth of
//! its tasks deleted; 16-edit chains at 10 000 and 50 000 tasks; and
//! `the_duration_sweeps_reach_the_cases_they_name`, which counts the cases the
//! cone itself solved. The 50 000-task chains add about 8 s to
//! `cargo test -p engine` (hub, load near 27, 2026-10-27).
//!
//! `GS_SEED=<n>` replays one seed; `GS_SEEDS=<count>` changes the fuzz count.

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
#[path = "../benches/cpm/duration.rs"]
mod duration;

use common::duration_chain::{self, Seen};
use common::equiv::assert_duration_cone;
use common::{check_invariants, fuzz, seeds};
use engine::{
    DAY, DurationCone, LagCalendar, Rng, Schedule, TaskId, generate, stress, stress_with_exceptions,
};

/// Edit random tasks' durations `edits` times, growing, shrinking, zeroing
/// and lengthening by days.
fn chain(s: &mut Schedule, rng: &mut Rng, edits: usize, label: &str) -> usize {
    let n = s.tasks().len() as i64;
    let mut sol = s.compute().expect("the base schedule computes");
    let mut applied = 0;
    for step in 0..edits {
        let task = TaskId(rng.range(0, n - 1) as u32);
        if s.is_deleted(task) {
            continue;
        }
        let old = s.tasks()[task.0 as usize].duration;
        let new = match rng.range(0, 5) {
            0 => 0,
            1 | 2 => (old + rng.range(-120, 120)).max(0),
            3 => old + rng.range(0, 3 * DAY),
            4 => rng.range(0, 2 * DAY),
            _ => old,
        };
        let ctx = format!("{label}, step {step}");
        applied += assert_duration_cone(s, &mut sol, task, new, &ctx) as usize;
    }
    check_invariants(s, &sol);
    applied
}

#[test]
fn fuzz_schedules_match_full_compute() {
    let mut applied = 0;
    for seed in seeds(400) {
        let mut s = fuzz::schedule(seed);
        let mut rng = Rng::new(seed ^ 0xd07a);
        applied += chain(&mut s, &mut rng, 12, &format!("fuzz {seed}"));
    }
    assert!(applied > 0);
}

#[test]
fn generated_and_stress_schedules_match_full_compute() {
    for seed in 0..6 {
        let mut s = generate(seed, 1_500);
        chain(&mut s, &mut Rng::new(seed), 40, &format!("generate {seed}"));
    }
    for seed in 0..2 {
        let mut s = stress(seed, 1_500).0;
        chain(&mut s, &mut Rng::new(seed), 30, &format!("stress {seed}"));
        let mut s = stress_with_exceptions(seed, 1_500).0;
        chain(
            &mut s,
            &mut Rng::new(seed),
            30,
            &format!("exceptions {seed}"),
        );
    }
}

/// The PR 1 workload at 10 000 tasks: the pinned target grows and shrinks back
/// with the finish fixed, and the cone ends on the original solution.
#[test]
fn the_pinned_duration_workload_matches_full_compute_and_restores() {
    let w = duration::Workload::new(10_000);
    let (mut s, mut sol) = (w.schedule, w.solution);
    let base = sol.clone();
    assert!(assert_duration_cone(
        &mut s,
        &mut sol,
        w.target,
        w.original + duration::DELTA,
        "grow"
    ));
    assert_eq!(sol.project_finish, base.project_finish);
    assert_ne!(sol, base);
    assert!(assert_duration_cone(
        &mut s, &mut sol, w.target, w.original, "shrink"
    ));
    assert_eq!(sol, base, "the original duration restores");
}

/// The bench's setup, as a test: at each bench size the pinned edit is solved
/// by the cone in both directions, not by a fallback that happens to return
/// the same dates.
#[test]
fn the_pinned_duration_workload_takes_the_cone_at_every_bench_size() {
    for (n, _) in duration::PINNED {
        let mut w = duration::Workload::new(n);
        let base = w.solution.clone();
        for longer in [true, false] {
            assert_eq!(
                w.step_cone(longer),
                DurationCone::Cone,
                "n = {n}: longer = {longer}"
            );
            assert_eq!(w.solution, w.schedule.clone().compute().unwrap());
        }
        assert_eq!(w.solution, base, "n = {n}: the original duration restores");
    }
}

// The equivalence matrix (gs#58 S4): the generated and stress schedules under
// every lag calendar, the eight-operation shuffled cycle of
// `common::duration_chain` (duration edits and SNET set and clear), a schedule
// with deleted tasks, and the sizes the demo page runs.

const FUZZ_SEEDS: u64 = 400;
const GENERATED_SEEDS: u64 = 6;
const STRESS_SEEDS: u64 = 2;
/// Two shuffled cycles of the eight operations per 16 edits.
const FUZZ_EDITS: usize = 16;
const GENERATED_EDITS: usize = 48;
const STRESS_EDITS: usize = 32;
const LAGS: [LagCalendar; 3] = [
    LagCalendar::Successor,
    LagCalendar::Predecessor,
    LagCalendar::TwentyFourHour,
];

fn fuzz_matrix() -> Seen {
    let mut seen = Seen::default();
    for seed in seeds(FUZZ_SEEDS) {
        let mut s = fuzz::schedule(seed);
        if s.compute().is_err() {
            continue;
        }
        let mut rng = Rng::new(seed ^ 0xd07b);
        seen.add(&duration_chain::run(
            &mut s,
            &mut rng,
            FUZZ_EDITS,
            &format!("fuzz {seed}"),
        ));
    }
    seen
}

/// The generated, stress and stress-with-exceptions schedules under `lag`.
fn lag_matrix(lag: LagCalendar) -> Seen {
    let mut seen = Seen::default();
    for seed in seeds(GENERATED_SEEDS) {
        let mut s = generate(seed, 1_500);
        s.set_lag_calendar(lag)
            .expect("the lag calendar is accepted");
        let label = format!("generate {seed}, {lag:?}");
        let mut rng = Rng::new(seed ^ 0xd07b);
        seen.add(&duration_chain::run(
            &mut s,
            &mut rng,
            GENERATED_EDITS,
            &label,
        ));
    }
    for seed in seeds(STRESS_SEEDS) {
        for (name, mut s) in [
            ("stress", stress(seed, 1_500).0),
            ("exceptions", stress_with_exceptions(seed, 1_500).0),
        ] {
            s.set_lag_calendar(lag)
                .expect("the lag calendar is accepted");
            let label = format!("{name} {seed}, {lag:?}");
            let mut rng = Rng::new(seed ^ 0xd07b);
            seen.add(&duration_chain::run(&mut s, &mut rng, STRESS_EDITS, &label));
        }
    }
    seen
}

fn lag_seen(lag: LagCalendar) -> &'static Seen {
    static SEEN: [std::sync::OnceLock<Seen>; 3] = [const { std::sync::OnceLock::new() }; 3];
    let i = LAGS.iter().position(|&l| l == lag).expect("a swept lag");
    SEEN[i].get_or_init(|| lag_matrix(lag))
}

fn fuzz_seen() -> &'static Seen {
    static SEEN: std::sync::OnceLock<Seen> = std::sync::OnceLock::new();
    SEEN.get_or_init(fuzz_matrix)
}

#[test]
fn the_cycle_matches_full_compute_on_fuzz_schedules() {
    let seen = fuzz_seen();
    eprintln!("fuzz cycle: {seen:?}");
}

#[test]
fn the_cycle_matches_full_compute_under_the_successor_lag_calendar() {
    eprintln!("successor cycle: {:?}", lag_seen(LagCalendar::Successor));
}

#[test]
fn the_cycle_matches_full_compute_under_the_predecessor_lag_calendar() {
    eprintln!(
        "predecessor cycle: {:?}",
        lag_seen(LagCalendar::Predecessor)
    );
}

#[test]
fn the_cycle_matches_full_compute_under_the_twenty_four_hour_lag_calendar() {
    eprintln!("24 h cycle: {:?}", lag_seen(LagCalendar::TwentyFourHour));
}

/// A chain over schedules with about a tenth of the tasks deleted: the cone
/// never queues a deleted task, and the deleted slots stay zero (the chain
/// checks each against a full compute at every step and again at the end).
#[test]
fn a_schedule_with_deleted_tasks_matches_full_compute() {
    let mut cone = 0;
    for (seed, lag) in LAGS.into_iter().enumerate() {
        let seed = seed as u64;
        let mut s = generate(seed, 1_500);
        s.set_lag_calendar(lag).unwrap();
        let mut rng = Rng::new(seed ^ 0xde_1e7e);
        let mut deleted = 0;
        for t in 0..s.tasks().len() {
            if rng.chance(0.1) && s.delete_task(TaskId(t as u32)).is_ok() {
                deleted += 1;
            }
        }
        assert!(deleted > 100, "seed {seed}: {deleted} tasks deleted");
        let label = format!("deleted {seed}, {lag:?}");
        let tally = duration_chain::run(&mut s, &mut rng, GENERATED_EDITS, &label);
        eprintln!("{label}: {deleted} deleted, {tally:?}");
        cone += tally.cone;
    }
    assert!(
        cone > 0,
        "the cone solved no edit on a schedule with deleted tasks"
    );
}

/// This sweep-level coverage check needs at least each default sweep size and
/// skips its assertions for a single-seed replay or a smaller configured run.
#[test]
fn the_duration_sweeps_reach_the_cases_they_name() {
    if let Some(seed) = std::env::var_os("GS_SEED") {
        eprintln!(
            "skipping coverage: GS_SEED={} requests a single-seed replay; the coverage thresholds need the default sweep minima (fuzz {FUZZ_SEEDS}, generated {GENERATED_SEEDS}, stress {STRESS_SEEDS})",
            seed.to_string_lossy()
        );
        return;
    }
    for (name, default) in [
        ("fuzz", FUZZ_SEEDS),
        ("generated", GENERATED_SEEDS),
        ("stress", STRESS_SEEDS),
    ] {
        let effective = seeds(default).len() as u64;
        if effective < default {
            eprintln!(
                "skipping coverage: GS_SEEDS={effective} is below the minimum {default} for the {name} sweep; the coverage thresholds need the default sweep"
            );
            return;
        }
    }

    let fuzz = fuzz_seen();
    eprintln!("fuzz: {fuzz:?}");
    assert!(fuzz.snet > 300, "{fuzz:?}");

    let mut total = Seen::default();
    for lag in LAGS {
        let seen = lag_seen(lag);
        eprintln!("{lag:?}: {seen:?}");
        // Every lag calendar reaches the cone, its early-date stop, its
        // backward-only change and a finish that moves.
        assert!(seen.cone > 50, "{lag:?}: {seen:?}");
        assert!(seen.stopped > 10, "{lag:?}: {seen:?}");
        assert!(seen.backward_only > 10, "{lag:?}: {seen:?}");
        assert!(seen.finish_moved > 0, "{lag:?}: {seen:?}");
        assert!(seen.snet > 50, "{lag:?}: {seen:?}");
        total.add(seen);
    }
    assert!(total.grew > 50 && total.shrank > 50, "{total:?}");
    assert!(
        total.to_milestone > 0 && total.from_milestone > 0,
        "{total:?}"
    );
}

/// The size of the demo page and past it: 16 edits of the shuffled cycle on
/// each schedule, the seed of the benches' large fixtures.
const LARGE_SEED: u64 = 20_261_020;
const LARGE_EDITS: usize = 16;

#[test]
fn a_chain_on_ten_thousand_generated_tasks_matches_full_compute() {
    let mut s = generate(2, 10_000);
    let mut rng = Rng::new(LARGE_SEED);
    let seen = duration_chain::run(&mut s, &mut rng, LARGE_EDITS, "generate 10k");
    eprintln!("generate 10k: {seen:?}");
    assert!(seen.cone > 0 && seen.snet > 0, "{seen:?}");
}

#[test]
fn a_chain_on_fifty_thousand_stress_tasks_matches_full_compute() {
    let mut s = stress(LARGE_SEED, 50_000).0;
    let mut rng = Rng::new(LARGE_SEED);
    let seen = duration_chain::run(&mut s, &mut rng, LARGE_EDITS, "stress 50k");
    eprintln!("stress 50k: {seen:?}");
    assert!(seen.cone > 0 && seen.snet > 0, "{seen:?}");
}

#[test]
fn a_chain_on_fifty_thousand_stress_tasks_with_exceptions_matches_full_compute() {
    let mut s = stress_with_exceptions(LARGE_SEED, 50_000).0;
    let mut rng = Rng::new(LARGE_SEED);
    let seen = duration_chain::run(&mut s, &mut rng, LARGE_EDITS, "exceptions 50k");
    eprintln!("exceptions 50k: {seen:?}");
    assert!(seen.cone > 0 && seen.snet > 0, "{seen:?}");
}
