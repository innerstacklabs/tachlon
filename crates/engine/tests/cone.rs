//! `Schedule::recompute_cone` against `Schedule::compute`: after any chain of
//! constraint edits, the incremental result equals a full recompute, task by
//! task (early and late dates, float, and so the critical flags) and in the
//! project finish.
//!
//! The chain is carried forward by the incremental path alone, never
//! refreshed from a full compute, so a wrong date that survives one step
//! feeds the next. Schedules cover calendars with holidays, milestones, SNET
//! constraints, all four link types and negative lags (the fuzz schedules),
//! plus the generated and stress schedules the benches use.
//!
//! `GS_SEED=<n>` replays one seed and skips sweep coverage. `GS_SEEDS=<count>`
//! changes every sweep size; coverage is checked only at or above each default.

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

use common::cone_chain::{Seen, first_difference, run_chain, stress_progress_sweep};
use common::{fuzz, seeds};
use engine::generate::add_limits;
use engine::{DAY, LagCalendar, Moved, Rng, TaskId, generate, stress};

const FUZZ_SEEDS: u64 = 400;
const GENERATED_SEEDS: u64 = 8;
const STRESS_SEEDS: u64 = 4;

#[expect(
    clippy::print_stderr,
    reason = "the sweep reports its seed count; cargo test shows it with --nocapture"
)]
fn fuzz_sweep(seed_list: Vec<u64>) -> Seen {
    eprintln!("sweep-run: fuzz ({} seeds)", seed_list.len());
    let mut seen = Seen::default();
    for seed in seed_list {
        let mut s = fuzz::schedule(seed);
        if s.compute().is_err() {
            continue;
        }
        let mut rng = Rng::new(seed ^ 0xc0_4e);
        seen.add(&run_chain(
            &mut s,
            &mut rng,
            12,
            &format!("fuzz seed {seed}"),
        ));
    }
    seen
}

#[expect(
    clippy::print_stderr,
    reason = "the sweep reports its seed count; cargo test shows it with --nocapture"
)]
fn generated_sweep(seed_list: Vec<u64>, lag: LagCalendar) -> Seen {
    eprintln!(
        "sweep-run: generated, {lag:?} lags ({} seeds)",
        seed_list.len()
    );
    let mut seen = Seen::default();
    for seed in seed_list {
        let mut s = generate(seed, 1_500);
        s.set_lag_calendar(lag)
            .expect("the lag calendar is accepted");
        let mut rng = Rng::new(seed ^ 0xc0_4e);
        seen.add(&run_chain(
            &mut s,
            &mut rng,
            40,
            &format!("generated seed {seed}"),
        ));
    }
    seen
}

#[expect(
    clippy::print_stderr,
    reason = "the sweep reports its seed count; cargo test shows it with --nocapture"
)]
fn stress_sweep(seed_list: Vec<u64>, lag: LagCalendar) -> Seen {
    eprintln!(
        "sweep-run: stress, {lag:?} lags ({} seeds)",
        seed_list.len()
    );
    let mut seen = Seen::default();
    for seed in seed_list {
        let (mut s, head) = stress(seed, 2_000);
        s.set_lag_calendar(lag)
            .expect("the lag calendar is accepted");
        let mut rng = Rng::new(seed ^ 0xc0_4e);
        seen.add(&run_chain(
            &mut s,
            &mut rng,
            20,
            &format!("stress seed {seed}"),
        ));
        // The head reaches nearly every task: its cone is the schedule.
        let mut sol = s.compute().expect("the schedule computes");
        for day in [9, 0, 21, 3] {
            s.set_snet(head, Some(day * DAY))
                .expect("the head task exists");
            let want = s.compute().expect("the schedule computes");
            s.recompute_cone(&mut sol, head)
                .expect("the cone recomputes");
            assert_eq!(
                first_difference(&sol, &want),
                None,
                "stress seed {seed}, head day {day}"
            );
        }
    }
    seen
}

fn fuzz_seen() -> &'static Seen {
    static SEEN: std::sync::OnceLock<Seen> = std::sync::OnceLock::new();
    SEEN.get_or_init(|| fuzz_sweep(seeds(FUZZ_SEEDS)))
}

fn generated_seen() -> &'static Seen {
    static SEEN: std::sync::OnceLock<Seen> = std::sync::OnceLock::new();
    SEEN.get_or_init(|| generated_sweep(seeds(GENERATED_SEEDS), LagCalendar::Successor))
}

fn stress_seen() -> &'static Seen {
    static SEEN: std::sync::OnceLock<Seen> = std::sync::OnceLock::new();
    SEEN.get_or_init(|| stress_sweep(seeds(STRESS_SEEDS), LagCalendar::Successor))
}

#[test]
fn cone_equals_full_recompute_on_fuzz_schedules() {
    let seen = fuzz_seen();
    eprintln!("fuzz: {seen:?}");
}

#[test]
fn cone_equals_full_recompute_on_generated_schedules() {
    let seen = generated_seen();
    eprintln!("generated: {seen:?}");
}

#[test]
fn cone_equals_full_recompute_on_stress_schedules() {
    let seen = stress_seen();
    eprintln!("stress: {seen:?}");
    let seen = stress_progress_sweep(seeds(STRESS_SEEDS));
    eprintln!("stress with progress: {seen:?}");
}

/// The drag-equals-full-compute property under the other lag calendars, which
/// the cone reads through the same `Pass` as the full compute: the generated
/// and stress schedules, whose tasks sit on several calendars, under
/// `Predecessor` and `TwentyFourHour`. The default sweeps above run
/// `Successor`; the fuzz sweep draws all four.
#[test]
fn cone_equals_full_recompute_under_the_predecessor_lag_calendar() {
    let seen = generated_sweep(seeds(GENERATED_SEEDS), LagCalendar::Predecessor);
    eprintln!("generated, predecessor: {seen:?}");
    let seen = stress_sweep(seeds(STRESS_SEEDS), LagCalendar::Predecessor);
    eprintln!("stress, predecessor: {seen:?}");
}

#[test]
fn cone_equals_full_recompute_under_the_twenty_four_hour_lag_calendar() {
    let seen = generated_sweep(seeds(GENERATED_SEEDS), LagCalendar::TwentyFourHour);
    eprintln!("generated, 24 h: {seen:?}");
    let seen = stress_sweep(seeds(STRESS_SEEDS), LagCalendar::TwentyFourHour);
    eprintln!("stress, 24 h: {seen:?}");
}

/// The lag calendar changes the dates of the generated schedules, so the two
/// tests above do not run the default result again under another name.
#[test]
fn the_lag_calendar_changes_the_generated_schedules() {
    let mut differs = [false; 2];
    for seed in 0..4 {
        let mut base = generate(seed, 1_500);
        let default = base.compute().unwrap();
        for (i, lag) in [LagCalendar::Predecessor, LagCalendar::TwentyFourHour]
            .into_iter()
            .enumerate()
        {
            let mut s = generate(seed, 1_500);
            s.set_lag_calendar(lag).unwrap();
            differs[i] |= s.compute().unwrap() != default;
        }
    }
    assert_eq!(differs, [true, true]);
}

/// The data date floors every unstarted task in the cone's forward pass as in the full compute (one
/// `Pass`), so after any chain of SNET edits under a data date the cone still equals
/// `compute`. The date is drawn per seed and changed between chains: a change of the data date is
/// solved by `compute` (it is not a cone), and the chain then continues from that solution.
/// `bitten` counts the schedules whose dates the data date moved, so the test cannot pass on
/// a data date no task feels.
#[test]
fn cone_equals_full_recompute_with_a_data_date() {
    eprintln!("sweep-run: fuzz with a data date");
    let (mut seen, mut bitten, mut schedules) = (Seen::default(), 0, 0);
    for seed in seeds(FUZZ_SEEDS) {
        let mut s = fuzz::schedule(seed);
        let Ok(plain) = s.compute() else { continue };
        let mut rng = Rng::new(seed ^ 0xda7a);
        for round in 0..3 {
            s.set_data_date(Some(rng.range(0, 10 * DAY)));
            let Ok(with) = s.compute() else {
                s.set_data_date(None);
                continue;
            };
            schedules += 1;
            bitten += (with != plain) as usize;
            seen.add(&run_chain(
                &mut s,
                &mut rng,
                6,
                &format!("fuzz seed {seed}, data date round {round}"),
            ));
        }
    }
    eprintln!("data date: {seen:?}, {bitten} of {schedules} schedules moved by it");
    if std::env::var_os("GS_SEED").is_none() && seeds(FUZZ_SEEDS).len() as u64 >= FUZZ_SEEDS {
        assert!(bitten * 4 > schedules, "{bitten} of {schedules}");
    }
}

/// The same property on the larger generated schedules, with the data date at the middle of the
/// project so that it floors many tasks and edited tasks land on both sides of it.
#[test]
fn cone_equals_full_recompute_with_a_data_date_on_generated_schedules() {
    for seed in seeds(GENERATED_SEEDS) {
        let mut s = generate(seed, 1_500);
        let mut rng = Rng::new(seed ^ 0xda7a);
        let plain = s.compute().expect("the base schedule computes");
        s.set_data_date(Some(plain.project_finish / 2));
        assert_ne!(s.compute().expect("computes"), plain, "seed {seed}");
        let seen = run_chain(
            &mut s,
            &mut rng,
            20,
            &format!("generated seed {seed}, data date"),
        );
        eprintln!("generated seed {seed}: {seen:?}");
        // A task with no predecessor is held only by the data date: an SNET set before it must not
        // move the task, in the cone as in the full compute.
        let has_pred: std::collections::HashSet<u32> =
            s.dependencies().iter().map(|d| d.succ.0).collect();
        let roots: Vec<TaskId> = (0..s.tasks().len() as u32)
            .filter(|i| !has_pred.contains(i))
            .map(TaskId)
            .take(10)
            .collect();
        assert!(
            !roots.is_empty(),
            "seed {seed}: no task without a predecessor"
        );
        let mut sol = s.compute().expect("computes");
        for root in roots {
            s.set_snet(root, Some(DAY)).unwrap();
            let want = s.compute().expect("computes");
            s.recompute_cone(&mut sol, root).unwrap();
            assert_eq!(first_difference(&sol, &want), None, "seed {seed}, {root}");
        }
    }
}

/// This sweep-level coverage check needs at least each default sweep size and
/// skips its assertions for a single-seed replay or a smaller configured run.
#[test]
fn the_sweeps_reach_the_cases_they_name() {
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
    assert!(fuzz.edits > 1000, "{fuzz:?}");
    assert!(fuzz.finish_moved > 100, "{fuzz:?}");
    assert!(fuzz.float_only > 100, "{fuzz:?}");
    assert!(fuzz.unchanged > 20, "{fuzz:?}");

    let generated = generated_seen();
    assert!(generated.finish_moved > 5, "{generated:?}");
    assert!(generated.float_only > 5, "{generated:?}");
    assert!(generated.wide > 10, "{generated:?}");

    let stress = stress_seen();
    assert!(stress.wide > 5, "{stress:?}");
}

/// A 10 000-task schedule: a chain of drags, as the demo page runs.
#[test]
fn cone_equals_full_recompute_on_a_large_schedule() {
    let mut s = generate(2, 10_000);
    let mut rng = Rng::new(99);
    let seen = run_chain(&mut s, &mut rng, 25, "10k");
    eprintln!("10k: {seen:?}");
}

/// The schedules the tests use contain what the task asks for: calendars
/// with holidays, milestones and SNET constraints, on a graph with every
/// link type.
#[test]
fn the_schedules_have_calendars_milestones_and_snet() {
    use engine::LinkType::*;
    for s in [generate(1, 1_500), stress(1, 1_500).0] {
        let used: std::collections::HashSet<u32> = s.tasks().iter().map(|t| t.calendar.0).collect();
        assert!(used.len() >= 3, "{} calendars used", used.len());
        assert!(s.tasks().iter().any(|t| t.duration == 0), "a milestone");
        assert!(s.tasks().iter().any(|t| t.snet.is_some()), "an SNET");
        for kind in [FS, SS, FF, SF] {
            assert!(s.dependencies().iter().any(|d| d.kind == kind), "{kind:?}");
        }
        assert!(s.dependencies().iter().any(|d| d.lag < 0), "a negative lag");
    }
    let (mut milestones, mut snets, mut negative) = (0, 0, 0);
    for seed in 0..400 {
        let s = fuzz::schedule(seed);
        milestones += s.tasks().iter().filter(|t| t.duration == 0).count();
        snets += s.tasks().iter().filter(|t| t.snet.is_some()).count();
        negative += s.dependencies().iter().filter(|d| d.lag < 0).count();
    }
    assert!(milestones > 100 && snets > 300 && negative > 300);
}

#[test]
fn a_solution_of_the_wrong_length_is_replaced_by_a_full_compute() {
    let mut s = generate(3, 300);
    let mut sol = s.compute().unwrap();
    s.set_snet(TaskId(7), Some(40 * DAY)).unwrap();
    let want = s.compute().unwrap();
    sol.dates.pop();
    s.recompute_cone(&mut sol, TaskId(7)).unwrap();
    assert_eq!(first_difference(&sol, &want), None);
}

#[test]
fn the_fallback_reports_every_task_with_no_old_dates() {
    let mut s = generate(3, 300);
    let mut sol = s.compute().unwrap();
    s.set_snet(TaskId(7), Some(40 * DAY)).unwrap();
    let want = s.compute().unwrap();
    sol.dates.pop();
    let mut moved = vec![common::equiv::moved_at(u32::MAX)];
    s.recompute_cone_moved(&mut sol, TaskId(7), &mut moved)
        .unwrap();
    // No old dates exist: `was_*` are the new dates, so nothing reads as moved.
    let expected: Vec<Moved> = (0..s.tasks().len())
        .map(|i| Moved {
            task: TaskId(i as u32),
            was_es: want.dates[i].es,
            was_ef: want.dates[i].ef,
        })
        .collect();
    assert_eq!(moved, expected);
    assert_eq!(sol, want);
}

#[test]
fn an_error_leaves_moved_empty() {
    let mut s = generate(3, 300);
    let mut sol = s.compute().unwrap();
    let before = sol.clone();
    let mut moved = vec![common::equiv::moved_at(7)];
    assert_eq!(
        s.recompute_cone_moved(&mut sol, TaskId(300), &mut moved),
        Err(engine::ScheduleError::UnknownTask(TaskId(300)))
    );
    assert_eq!(moved, Vec::new());
    assert_eq!(sol, before);
}

/// The same property on schedules whose tasks carry date limits: a limit that no date meets gives
/// a late start before the early start, which the cone must leave to the full late pass, as the
/// full compute does. `unmet` counts the schedules with a negative float, so the test cannot pass
/// on limits that are all met.
#[test]
fn cone_equals_full_recompute_with_date_limits() {
    let (mut unmet, mut rng) = (0, Rng::new(0xc0_4e));
    for seed in seeds(FUZZ_SEEDS) {
        let mut s = fuzz::schedule(seed);
        add_limits(&mut s, seed, 0.4, 20 * DAY);
        let Ok(sol) = s.compute() else { continue };
        unmet += sol.dates.iter().any(|d| d.float < 0) as usize;
        run_chain(&mut s, &mut rng, 8, &format!("limited fuzz seed {seed}"));
    }
    for seed in seeds(GENERATED_SEEDS) {
        let mut s = generate(seed, 1_500);
        add_limits(&mut s, seed, 0.05, 150 * DAY);
        unmet += s.compute().unwrap().dates.iter().any(|d| d.float < 0) as usize;
        run_chain(
            &mut s,
            &mut rng,
            20,
            &format!("limited generated seed {seed}"),
        );
    }
    if std::env::var_os("GS_SEED").is_none() && seeds(FUZZ_SEEDS).len() as u64 >= FUZZ_SEEDS {
        assert!(unmet > 100, "{unmet} schedules with a negative float");
    }
}
