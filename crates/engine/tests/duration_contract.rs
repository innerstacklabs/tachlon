//! What a duration change does to a solution, pinned against a full
//! `Schedule::compute` (gs#58 PR 1). A two-pass duration update must match
//! these; today they hold because every edit recomputes in full.
//!
//! Hand-built schedules use the 24/7 calendar, so 60 working minutes are 60
//! wall-clock minutes and every date below is exact.

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

use common::{check_invariants, cont_cal, dep, task};
use engine::{LinkType, Rng, Schedule, Solution, TaskId, generate};

const H: i64 = 60;

/// A schedule on one 24/7 calendar; tasks are `(duration)` in order.
fn build(durations: &[i64]) -> (Schedule, Vec<TaskId>) {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let ids = durations
        .iter()
        .map(|&d| task(&mut s, cal, d, None))
        .collect();
    (s, ids)
}

/// Set a duration and recompute in full, checking the invariants.
fn with_duration(s: &mut Schedule, t: TaskId, d: i64) -> Solution {
    s.set_duration(t, d).expect("task exists");
    let sol = s.compute().expect("computes");
    check_invariants(s, &sol);
    sol
}

/// `P -> T -> S` (FS) beside a long task `X` that sets the finish.
fn chain_beside_long() -> (Schedule, [TaskId; 4]) {
    let (mut s, id) = build(&[120, 120, 120, 1_000]);
    dep(&mut s, id[0], id[1], LinkType::FS, 0);
    dep(&mut s, id[1], id[2], LinkType::FS, 0);
    (s, [id[0], id[1], id[2], id[3]])
}

/// The gs#48 trace, seed 0, step 0 of the duration sweep, is NOT reproduced.
/// The sweep is not on main, so this draws step 0 the way `cone.rs` does
/// (task from `Rng::new(seed ^ 0xc04e)` on `generate(0, 1500)`, +60 minutes)
/// and records what that edit does here: the finish stays fixed and the
/// task's late start moves, but no predecessor's float or late dates move
/// (the predecessor is held by other successors). The ruling's predecessor effect is pinned on hand-built schedules below instead.
#[test]
fn gs48_seed0_step0_as_drawn_here_moves_late_dates_but_no_predecessor() {
    let mut s = generate(0, 1_500);
    let base = s.compute().expect("computes");
    let n = s.tasks().len() as i64;
    let t = TaskId(Rng::new(0xc0_4e).range(0, n - 1) as u32);
    let old = s.tasks()[t.0 as usize].duration;
    let grown = with_duration(&mut s, t, old + H);
    assert_eq!(grown.project_finish, base.project_finish, "finish fixed");
    let (before, after) = (base.get(t), grown.get(t));
    assert_eq!(
        (after.ls, after.lf),
        (before.ls - H, before.lf),
        "the task's late start moves"
    );
    for d in s.dependencies().iter().filter(|d| d.succ == t) {
        assert_eq!(
            base.get(d.pred),
            grown.get(d.pred),
            "predecessor {:?}",
            d.pred
        );
    }
    assert_eq!(with_duration(&mut s, t, old), base, "shrinking restores");
}

#[test]
fn growing_a_task_keeps_a_fixed_finish_and_moves_both_neighbours() {
    let (mut s, [p, t, succ, x]) = chain_beside_long();
    let base = s.compute().unwrap();
    let grown = with_duration(&mut s, t, 120 + H);
    assert_eq!(grown.project_finish, base.project_finish);
    assert_eq!(grown.get(succ).es, base.get(succ).es + H, "successor later");
    assert_eq!(grown.get(p).es, base.get(p).es, "predecessor's early dates");
    assert_eq!(
        grown.get(p).lf,
        base.get(p).lf - H,
        "predecessor's late dates"
    );
    assert_eq!(grown.get(p).float, base.get(p).float - H);
    assert_eq!(grown.get(x), base.get(x), "the parallel task is untouched");
}

#[test]
fn shrinking_is_the_mirror_of_growing() {
    let (mut s, [p, t, succ, _]) = chain_beside_long();
    let base = s.compute().unwrap();
    let shorter = with_duration(&mut s, t, 120 - H);
    assert_eq!(shorter.project_finish, base.project_finish);
    assert_eq!(shorter.get(succ).es, base.get(succ).es - H);
    assert_eq!(shorter.get(p).lf, base.get(p).lf + H);
    assert_eq!(shorter.get(p).float, base.get(p).float + H);
    assert_eq!(with_duration(&mut s, t, 120), base, "and back again");
}

#[test]
fn ff_successor_starts_earlier_when_it_grows() {
    let (mut s, [p, t]) = {
        let (s, id) = build(&[300, 120]);
        (s, [id[0], id[1]])
    };
    dep(&mut s, p, t, LinkType::FF, 0);
    let base = s.compute().unwrap();
    assert_eq!(base.get(t).ef, base.get(p).ef, "T finishes with P");
    let grown = with_duration(&mut s, t, 120 + H);
    assert_eq!(grown.get(t).ef, base.get(t).ef, "the finish is held by P");
    assert_eq!(grown.get(t).es, base.get(t).es - H, "the start moves");
    let shorter = with_duration(&mut s, t, 120 - H);
    assert_eq!(shorter.get(t).es, base.get(t).es + H);
}

#[test]
fn sf_successor_starts_earlier_when_it_grows() {
    let (mut s, id) = build(&[300, 600]);
    let (p, t) = (id[0], id[1]);
    s.set_snet(p, Some(1_000)).unwrap();
    dep(&mut s, p, t, LinkType::SF, 0);
    let base = s.compute().unwrap();
    assert_eq!(base.get(t).ef, base.get(p).es, "T finishes at P's start");
    let grown = with_duration(&mut s, t, 600 + H);
    assert_eq!(grown.get(t).ef, base.get(t).ef);
    assert_eq!(grown.get(t).es, base.get(t).es - H);
    let shorter = with_duration(&mut s, t, 600 - H);
    assert_eq!(shorter.get(t).es, base.get(t).es + H);
}

#[test]
fn a_milestone_becomes_a_task_and_back() {
    let (mut s, [p, t, succ, _]) = chain_beside_long();
    s.set_duration(t, 0).unwrap();
    let milestone = s.compute().unwrap();
    assert_eq!(milestone.get(t).es, milestone.get(t).ef);
    let task_now = with_duration(&mut s, t, H);
    assert_eq!(task_now.get(t).ef, task_now.get(t).es + H);
    assert_eq!(task_now.get(succ).es, milestone.get(succ).es + H);
    assert_eq!(task_now.get(p).lf, milestone.get(p).lf - H);
    assert_eq!(task_now.project_finish, milestone.project_finish);
    assert_eq!(
        with_duration(&mut s, t, 0),
        milestone,
        "and a milestone again"
    );
}

#[test]
fn a_critical_task_moves_the_finish_both_ways() {
    let (mut s, id) = build(&[120, 300, 200]);
    let (a, b, side) = (id[0], id[1], id[2]);
    dep(&mut s, a, b, LinkType::FS, 0);
    let base = s.compute().unwrap();
    assert_eq!(base.project_finish, 420);
    assert!(base.get(b).is_critical());
    let grown = with_duration(&mut s, b, 300 + H);
    assert_eq!(grown.project_finish, 480, "growth moves the finish");
    assert_eq!(grown.get(side).float, base.get(side).float + H);
    let shorter = with_duration(&mut s, b, 300 - H);
    assert_eq!(shorter.project_finish, 360, "shrinkage moves it back");
    // Shrinking past the parallel path: the side task now sets the finish.
    let much = with_duration(&mut s, b, 10);
    assert_eq!(much.project_finish, 200);
    assert!(much.get(side).is_critical());
    assert_eq!(with_duration(&mut s, b, 300), base);
}

#[test]
fn the_pinned_targets_qualify_and_nothing_earlier_does() {
    for (n, pinned) in duration::PINNED {
        let (mut s, _) = engine::stress(duration::SEED, n);
        let base = s.compute().unwrap();
        let target = TaskId(pinned);
        assert!(duration::qualifies(&mut s, &base, target), "stress {n}");
        assert!(
            (0..pinned).all(|t| !duration::qualifies(&mut s, &base, TaskId(t))),
            "stress {n}: a task before the pin qualifies"
        );
        assert_eq!(
            s.compute().unwrap(),
            base,
            "qualifying restores the schedule"
        );
        let mut w = duration::Workload::new(n);
        w.step(true);
        assert_ne!(w.moved, Vec::new());
        assert_eq!(w.solution.project_finish, base.project_finish);
        assert_eq!(w.solution, w.schedule.compute().unwrap());
        w.step(false);
        assert_eq!(w.solution, base, "n = {n}: the original duration restores");
    }
}
