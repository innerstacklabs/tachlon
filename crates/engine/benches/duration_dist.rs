//! How often a duration edit falls back to a full compute, and what each
//! outcome costs (gs#519). Not a Criterion bench: it draws a fixed, seeded set
//! of 200 tasks on `stress(20261020, 50_000)`, grows each by 60 working
//! minutes, and prints, per `DurationCone` class, the count (the frequency),
//! the p50 and p95 of the cone call, and of the tasks the cone examined and the
//! tasks whose early dates changed. The control is a full `compute` of the same
//! edit, which is what `duration_full` times.
//! Run: `cargo bench -p engine --bench duration_dist`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

#[path = "cpm/duration.rs"]
mod duration;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use duration::{DELTA, SEED};
use engine::test_hooks::ADMITTED;
use engine::{Moved, Rng, TaskId, stress};

const N: usize = 50_000;
const DRAWN: usize = 200;

/// One edit: the cone call and the control in microseconds, the tasks examined
/// and the tasks whose early dates changed.
type Row = [f64; 4];

/// Nearest-rank percentile of column `i` of `rows`.
fn pct(rows: &[Row], i: usize, p: usize) -> f64 {
    let mut v: Vec<f64> = rows.iter().map(|r| r[i]).collect();
    v.sort_by(f64::total_cmp);
    v[(v.len() * p).div_ceil(100).max(1) - 1]
}

#[expect(
    clippy::print_stdout,
    reason = "the bench reports its measurements on stdout"
)]
fn main() {
    let (mut sched, _) = stress(SEED, N);
    let base = sched.compute().expect("stress fixture computes");
    let mut rng = Rng::new(SEED);
    let mut drawn = BTreeSet::new();
    while drawn.len() < DRAWN {
        drawn.insert(rng.range(0, N as i64 - 1) as u32);
    }
    let mut by_class = BTreeMap::<String, Vec<Row>>::new();
    let mut moved = Vec::<Moved>::new();
    for &id in &drawn {
        let task = TaskId(id);
        let original = sched.tasks()[id as usize].duration;
        sched
            .set_duration(task, original + DELTA)
            .expect("task exists");
        let mut sol = base.clone();
        let t = Instant::now();
        let class = sched.recompute_duration_cone_moved(&mut sol, task, &mut moved);
        let cone_us = t.elapsed().as_secs_f64() * 1e6;
        let examined = ADMITTED.get() as f64;
        let t = Instant::now();
        let full = sched.compute().expect("computes");
        let full_us = t.elapsed().as_secs_f64() * 1e6;
        assert_eq!(sol, full, "task {id}: the cone differs from a full compute");
        sched.set_duration(task, original).expect("task exists");
        let row = [cone_us, full_us, examined, moved.len() as f64];
        let class = format!("{:?}", class.expect("computes"));
        by_class.entry(class).or_default().push(row);
    }
    println!(
        "{DRAWN} tasks of stress({SEED}, {N}), Rng::new({SEED}), each +{DELTA} working minutes"
    );
    let all: Vec<Row> = by_class.values().flatten().copied().collect();
    let control = pct(&all, 1, 50);
    println!(
        "control duration_full: p50 {control:.0} us, p95 {:.0} us",
        pct(&all, 1, 95)
    );
    let mut gave_up: Vec<Row> = Vec::new();
    for (class, rows) in &by_class {
        let q = |i| format!("{:.0}/{:.0}", pct(rows, i, 50), pct(rows, i, 95));
        let share = rows.len() as f64 * 100.0 / DRAWN as f64;
        let (n, cone, full, seen, changed) = (rows.len(), q(0), q(1), q(2), q(3));
        println!(
            "{class}: count {n} ({share:.1}%), cone {cone} us, full {full} us, examined {seen}, changed {changed} (p50/p95)"
        );
        if class != "Cone" {
            gave_up.extend(rows);
        }
    }
    let cone = by_class.get("Cone").map(|rows| pct(rows, 0, 50));
    let Some(cone) = cone.filter(|_| !gave_up.is_empty()) else {
        println!(
            "crossover: not computed, {} of {DRAWN} edits fell back",
            gave_up.len()
        );
        return;
    };
    // The cone path beats the full compute on average while
    // (1 - f) * cone + f * fell_back < control, f the share that fall back.
    let fell_back = pct(&gave_up, 0, 50);
    let share = ((control - cone) / (fell_back - cone) * 100.0).min(100.0);
    let n = gave_up.len();
    println!(
        "crossover: cone call p50 {cone:.0} us when it solves the edit, {fell_back:.0} us when it falls back ({n} edits); the cone path is faster on average while fewer than {share:.1}% of edits fall back (100.0 = even if all do)"
    );
}
