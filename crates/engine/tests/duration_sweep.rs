//! The fuzz sweep that reaches the duration cone and every fallback reason
//! (gs#58 S2, gs#518).
//!
//! `duration_cone.rs` proves that the incremental result equals a full compute,
//! but not that the cone ran: on a 2- to 20-task schedule the production
//! limit is one or two tasks, so most edits fall back and a result that
//! equals the full compute says nothing about the cone's two passes. This
//! file runs `fuzz::schedule` seeds 0..400, 64 edits each, at two settings of
//! the `test-hooks` limit override, reads the returned `DurationCone`, and
//! asserts what was reached:
//!
//! * raised limit (1,000 admitted tasks, the production inspection cap): the
//!   cone runs for each of the four lag calendars, for a share of the edits
//!   that no longer falls below `CONE_SHARE_FLOOR_PERCENT`, and `FinishMove`
//!   occurs;
//! * low limit (3 admitted tasks, 8 inspections): `ForwardLimit`,
//!   `BackwardLimit` and `EdgeLimit` each occur. A 20-task schedule cannot
//!   reach the real 16,384-inspection cap, so the low setting is the only way
//!   to reach `EdgeLimit` here; `edge_limit_falls_back_on_a_dense_fan_out`
//!   (unit tests) stays the real-cap test.
//!
//! `EsAfterLs` occurs zero times here: from an exact base it arises only where
//! the full compute also fails (the refused arm of `assert_duration_cone`),
//! and `an_early_start_past_its_late_start_falls_back_to_the_full_solve`
//! (unit tests) covers it.
//!
//! Every edit, at both settings, also goes through the equivalence check of
//! `common::equiv`: the solution and `moved` equal a full compute.
//!
//! The edit mix is the one `chain` uses in `duration_cone.rs`: zero, plus or
//! minus 120 minutes, plus 0 to 3 days, a random 0 to 2 days, and no change.
//!
//! `GS_SEED=<n>` replays one seed (the coverage asserts then do not apply);
//! `GS_SEEDS=<count>` changes the seed count. Run with `-- --nocapture` to
//! print the outcome tally.

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

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use common::equiv::assert_duration_cone_outcome;
use common::{fuzz, seeds};
use engine::test_hooks::{LIMITS, Limits, MAX_INSPECTIONS, OMIT_BACKWARD};
use engine::{DAY, DurationCone, FallbackReason, LagCalendar, Rng, TaskId};

/// Seeds in the full sweep, and edits per seed.
const SEEDS: u64 = 400;
const EDITS: usize = 64;

/// A floor on the share of changed durations that the cone solves at the
/// raised limit. Measured on 2026-10-27: 14,045 of 19,176, 73.2% (the other
/// 5,131 moved the project finish; 6,424 more edits kept the duration they
/// had and are not counted). The sweep is deterministic, so the margin below
/// the measured share is for a change to the edit mix, not for noise.
const CONE_SHARE_FLOOR_PERCENT: usize = 65;

const RAISED: Limits = Limits {
    admitted: 1_000,
    inspections: MAX_INSPECTIONS,
};
const LOW: Limits = Limits {
    admitted: 3,
    inspections: 8,
};

/// Puts the production limits back, and clears the omission, when a test ends
/// or panics. The hooks are thread-local, so this affects only its own test.
struct Hooks;

impl Hooks {
    fn limits(limits: Limits) -> Hooks {
        LIMITS.set(Some(limits));
        Hooks
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        LIMITS.set(None);
        OMIT_BACKWARD.set(None);
    }
}

/// How often each outcome occurred. A key is the outcome (`Cone`,
/// `Fallback(FinishMove)`, `Replaced`, `Refused`), and a `Cone` also counts
/// under `Cone/<lag calendar>`. An edit to the duration it already had counts
/// only as `NoChange/<outcome>`.
#[derive(Debug, Default)]
struct Tally(BTreeMap<String, usize>);

impl Tally {
    fn add(&mut self, key: String) {
        *self.0.entry(key).or_default() += 1;
    }

    fn count(&self, key: &str) -> usize {
        self.0.get(key).copied().unwrap_or(0)
    }

    /// The edits that changed a duration and were not refused.
    fn applied(&self) -> usize {
        self.0
            .iter()
            .filter(|(k, _)| !k.contains('/') && *k != "Refused")
            .map(|(_, n)| n)
            .sum()
    }

    #[expect(
        clippy::print_stdout,
        reason = "the sweep prints its tally; cargo test shows it with --nocapture"
    )]
    fn print(&self, label: &str) {
        println!("{label}: {} applied edits", self.applied());
        for (key, n) in &self.0 {
            println!("  {key:<28} {n}");
        }
    }
}

fn lag_name(lag: LagCalendar) -> &'static str {
    match lag {
        LagCalendar::Successor => "Successor",
        LagCalendar::Predecessor => "Predecessor",
        LagCalendar::TwentyFourHour => "TwentyFourHour",
        LagCalendar::Fixed(_) => "Fixed",
    }
}

/// One seed: `EDITS` random duration edits on `fuzz::schedule(seed)`, each
/// checked against a full compute, each outcome added to `tally`. Panics on
/// any difference.
fn run_seed(seed: u64, tally: &mut Tally) {
    let mut s = fuzz::schedule(seed);
    let mut rng = Rng::new(seed ^ 0xd07a);
    let n = s.tasks().len() as i64;
    let lag = lag_name(s.lag_calendar());
    let mut sol = s.compute().expect("the base schedule computes");
    for step in 0..EDITS {
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
        let label = format!("sweep {seed}, step {step}");
        match assert_duration_cone_outcome(&mut s, &mut sol, task, new, &label) {
            // An edit to the same duration moves nothing and is a cone trivially,
            // so it is counted apart and the share below is of real changes.
            Some(how) if new == old => tally.add(format!("NoChange/{how:?}")),
            Some(how) => {
                if how == DurationCone::Cone {
                    tally.add(format!("Cone/{lag}"));
                }
                tally.add(format!("{how:?}"));
            }
            None => tally.add("Refused".to_string()),
        }
    }
}

/// The sweep at `limits`. Returns the tally, and whether it ran every seed
/// (a replay of one seed does not, and the coverage asserts then do not apply).
fn sweep(limits: Limits) -> (Tally, bool) {
    let _hooks = Hooks::limits(limits);
    let seeds = seeds(SEEDS);
    let mut tally = Tally::default();
    for &seed in &seeds {
        run_seed(seed, &mut tally);
    }
    (tally, seeds.len() as u64 == SEEDS)
}

#[test]
fn the_raised_limit_reaches_the_cone_for_every_lag_calendar() {
    let (tally, full) = sweep(RAISED);
    tally.print("raised limit");
    if !full {
        return;
    }
    for lag in ["Successor", "Predecessor", "TwentyFourHour", "Fixed"] {
        assert!(
            tally.count(&format!("Cone/{lag}")) > 0,
            "the cone never solved an edit on a {lag} lag calendar: {tally:?}"
        );
    }
    let finish = format!("{:?}", DurationCone::Fallback(FallbackReason::FinishMove));
    assert!(
        tally.count(&finish) > 0,
        "no edit moved the finish: {tally:?}"
    );
    let (cone, applied) = (tally.count("Cone"), tally.applied());
    assert!(
        cone * 100 >= applied * CONE_SHARE_FLOOR_PERCENT,
        "the cone solved {cone} of {applied} edits, under {CONE_SHARE_FLOOR_PERCENT}%: {tally:?}"
    );
}

#[test]
fn the_low_limit_reaches_every_limit_fallback() {
    let (tally, full) = sweep(LOW);
    tally.print("low limit");
    if !full {
        return;
    }
    for reason in [
        FallbackReason::ForwardLimit,
        FallbackReason::BackwardLimit,
        FallbackReason::EdgeLimit,
    ] {
        let key = format!("{:?}", DurationCone::Fallback(reason));
        assert!(
            tally.count(&key) > 0,
            "no edit fell back with {reason:?}: {tally:?}"
        );
    }
}

/// The sweep can fail: with the backward pass missing one predecessor, some
/// seed at the raised limit stops matching the full compute. Without this the
/// equivalence check inside the sweep could be loosened and nothing would turn
/// red. The seed it finds is printed.
#[test]
fn a_backward_pass_that_misses_a_predecessor_fails_the_raised_sweep() {
    let _hooks = Hooks::limits(RAISED);
    for seed in 0..SEEDS {
        let n = fuzz::schedule(seed).tasks().len() as u32;
        for omit in 0..n {
            OMIT_BACKWARD.set(Some(TaskId(omit)));
            let missed = catch_unwind(AssertUnwindSafe(|| run_seed(seed, &mut Tally::default())));
            OMIT_BACKWARD.set(None);
            if missed.is_err() {
                println!("seed {seed}, omitting task {omit}: the sweep failed");
                // The same seed passes with the pass intact.
                run_seed(seed, &mut Tally::default());
                return;
            }
        }
    }
    panic!("no seed noticed a predecessor missing from the backward pass");
}
