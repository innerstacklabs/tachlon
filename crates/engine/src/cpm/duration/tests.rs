//! The duration cone against a full compute, and each fallback forced.
//!
//! Every case builds the schedule, solves it, changes one duration and runs
//! the cone on the old solution; `run` checks the result and the early-date
//! `moved` list against a full compute of a clone, and returns how the cone
//! ended, so a case cannot pass only because it fell back every time.

use super::seam::{ADMITTED, ENQUEUED, FAIL_BACKWARD, FAIL_FALLBACK, LAST};
use super::*;
use crate::calendar::{Calendar, DAY, DayPeriod};
use crate::generate::{generate, stress};
use crate::model::{CalendarId, Dependency, LinkType, Task};

const H: i64 = 60;
const LONG: i64 = 10_000_000;

fn always() -> Calendar {
    let day: Vec<DayPeriod> = vec![(0, 1440)];
    Calendar::new(std::array::from_fn(|_| day.clone()), &[]).unwrap()
}

fn add(s: &mut Schedule, cal: CalendarId, duration: i64) -> TaskId {
    let task = Task {
        duration,
        calendar: cal,
        snet: None,
        limit: crate::model::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    s.add_task(task).unwrap()
}

fn link(s: &mut Schedule, pred: TaskId, succ: TaskId, kind: LinkType, lag: i64) {
    let dep = Dependency {
        pred,
        succ,
        kind,
        lag,
    };
    s.add_dependency(dep).unwrap();
}

/// `len` tasks of 10 minutes in a finish-to-start chain, beside one very long
/// task that holds the project finish. Returns the chain.
fn chain_beside_long(len: usize) -> (Schedule, Vec<TaskId>) {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let ids: Vec<TaskId> = (0..len).map(|_| add(&mut s, cal, 10)).collect();
    for w in ids.windows(2) {
        link(&mut s, w[0], w[1], LinkType::FS, 0);
    }
    add(&mut s, cal, LONG);
    (s, ids)
}

/// The tasks whose early dates differ, each with its early dates in `old`
/// (what `Moved` must carry), in topological order.
fn early_diff(s: &Schedule, old: &Solution, new: &Solution) -> Vec<Moved> {
    let g = s.graph.as_ref().expect("computed");
    let early = |a: &Solution, i: u32| (a.dates[i as usize].es, a.dates[i as usize].ef);
    let moved = g.order.iter().filter(|&&i| early(old, i) != early(new, i));
    moved
        .map(|&i| Moved::was(i, &old.dates[i as usize]))
        .collect()
}

/// A stale entry for the list a call is handed: the call must clear it.
fn stale(task: u32) -> Vec<Moved> {
    vec![Moved {
        task: TaskId(task),
        was_es: 0,
        was_ef: 0,
    }]
}

struct Run {
    outcome: DurationCone,
    moved: Vec<TaskId>,
}

/// Set `task`'s duration, run the cone from `base`, and check it equals a full
/// compute, `moved` included. Leaves the new duration in place.
fn run(s: &mut Schedule, base: &Solution, task: TaskId, duration: i64) -> Run {
    s.set_duration(task, duration).unwrap();
    let want = s.clone().compute().unwrap();
    let (mut sol, mut moved) = (base.clone(), stale(999_999));
    let outcome = s
        .recompute_duration_cone_moved(&mut sol, task, &mut moved)
        .unwrap();
    assert_eq!(sol, want, "{task}");
    assert_eq!(moved, early_diff(s, base, &want), "{task}: moved");
    let named = match outcome {
        DurationCone::Fallback(reason) => Some(reason),
        DurationCone::Cone | DurationCone::Replaced => None,
    };
    assert_eq!(LAST.get(), named, "{task}: the seam names the fallback");
    Run {
        outcome,
        moved: moved.iter().map(|m| m.task).collect(),
    }
}

/// Force the fallback `reason`'s full compute to fail: the cone errs and
/// leaves the solution and `moved` as an error should.
fn assert_failing_fallback_changes_nothing(
    s: &mut Schedule,
    base: &Solution,
    task: TaskId,
    duration: i64,
    reason: FallbackReason,
) {
    s.set_duration(task, duration).unwrap();
    FAIL_FALLBACK.set(Some(reason));
    let (mut sol, mut moved) = (base.clone(), stale(7));
    let got = s.recompute_duration_cone_moved(&mut sol, task, &mut moved);
    FAIL_FALLBACK.set(None);
    assert!(got.is_err(), "{reason:?}: the fallback was not reached");
    assert_eq!(LAST.get(), Some(reason));
    assert_eq!(&sol, base, "{reason:?}: the prior solution changed");
    assert!(moved.is_empty(), "{reason:?}: moved not empty");
    // The same call succeeds once the fault is gone.
    let want = s.clone().compute().unwrap();
    s.recompute_duration_cone_moved(&mut sol, task, &mut moved)
        .unwrap();
    assert_eq!(sol, want, "{reason:?}: recovery");
}

#[test]
fn a_small_cone_is_solved_by_the_cone_and_equals_full_compute() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let id: Vec<TaskId> = (0..30).map(|_| add(&mut s, cal, 100)).collect();
    // p -> t -> q beside 27 unrelated tasks and one long one.
    link(&mut s, id[0], id[1], LinkType::FS, 0);
    link(&mut s, id[1], id[2], LinkType::FS, 0);
    add(&mut s, cal, LONG);
    let base = s.compute().unwrap();
    let grown = run(&mut s, &base, id[1], 100 + H);
    assert_eq!(grown.outcome, DurationCone::Cone);
    assert_eq!(grown.moved, [id[1], id[2]], "t and its successor");
    let shrunk = run(&mut s, &base, id[1], 40);
    assert_eq!(shrunk.outcome, DurationCone::Cone);
    // An edit that changes nothing early: only late dates and float move.
    let milestone = run(&mut s, &base, id[1], 0);
    assert_eq!(milestone.outcome, DurationCone::Cone);
}

#[test]
fn a_backward_only_change_is_not_in_moved() {
    // Growing the last task of a chain leaves every early date but its own
    // finish; the predecessors' late dates move and are not reported.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let id: Vec<TaskId> = (0..3).map(|_| add(&mut s, cal, 100)).collect();
    link(&mut s, id[0], id[1], LinkType::FS, 0);
    link(&mut s, id[1], id[2], LinkType::FS, 0);
    for _ in 0..40 {
        add(&mut s, cal, 10);
    }
    add(&mut s, cal, LONG);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, id[2], 100 + H);
    assert_eq!(r.outcome, DurationCone::Cone);
    assert_eq!(r.moved, [id[2]]);
    let mut sol = base.clone();
    s.recompute_duration_cone_moved(&mut sol, id[2], &mut Vec::new())
        .unwrap();
    assert_eq!(sol.get(id[0]).lf, base.get(id[0]).lf - H);
}

#[test]
fn forward_limit_falls_back_and_reports_the_early_diff() {
    // 41 live tasks: the limit is 5 admitted; a chain of 40 moves them all.
    let (mut s, chain) = chain_beside_long(40);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, chain[0], 10 + H);
    assert_eq!(
        r.outcome,
        DurationCone::Fallback(FallbackReason::ForwardLimit)
    );
    assert_eq!(r.moved, chain);
    assert_failing_fallback_changes_nothing(
        &mut s,
        &base,
        chain[0],
        10 + H,
        FallbackReason::ForwardLimit,
    );
}

#[test]
fn backward_limit_falls_back_after_the_root_was_written_by_both_passes() {
    let (mut s, chain) = chain_beside_long(40);
    let base = s.compute().unwrap();
    let last = *chain.last().unwrap();
    let r = run(&mut s, &base, last, 10 + H);
    assert_eq!(
        r.outcome,
        DurationCone::Fallback(FallbackReason::BackwardLimit)
    );
    assert_eq!(r.moved, [last], "the root's early finish moved");
    assert_failing_fallback_changes_nothing(
        &mut s,
        &base,
        last,
        10 + H,
        FallbackReason::BackwardLimit,
    );
}

#[test]
fn edge_limit_falls_back_on_a_dense_fan_out() {
    // One hub with 20 000 successors: few tasks, too many dependencies.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let hub = add(&mut s, cal, 10);
    for _ in 0..20_000 {
        let t = add(&mut s, cal, 10);
        link(&mut s, hub, t, LinkType::FS, 0);
    }
    add(&mut s, cal, LONG);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, hub, 10 + H);
    assert_eq!(r.outcome, DurationCone::Fallback(FallbackReason::EdgeLimit));
    assert_eq!(r.moved.len(), 20_001);
    assert_failing_fallback_changes_nothing(&mut s, &base, hub, 10 + H, FallbackReason::EdgeLimit);
}

/// A hub and `wide` other tasks, linked out from the hub (`fan_in` false) or
/// into it, beside one long task. 1 002 live tasks give a limit of 101.
fn hub(wide: usize, fan_in: bool) -> (Schedule, TaskId) {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let hub = add(&mut s, cal, 10);
    for _ in 0..wide {
        let t = add(&mut s, cal, 10);
        let (p, q) = if fan_in { (t, hub) } else { (hub, t) };
        link(&mut s, p, q, LinkType::FS, 0);
    }
    add(&mut s, cal, LONG);
    (s, hub)
}

#[test]
fn a_wide_fan_out_below_the_edge_cap_stops_discovery_at_the_limit() {
    let (mut s, hub) = hub(1_000, false);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, hub, 10 + H);
    assert_eq!(
        r.outcome,
        DurationCone::Fallback(FallbackReason::ForwardLimit)
    );
    assert_eq!(r.moved.len(), 1_001, "the oracle's early diff");
    // The hub and 100 successors; the 101st is refused before it is enqueued.
    assert_eq!(ADMITTED.get(), 101);
    assert_eq!(ENQUEUED.get(), 101);
}

#[test]
fn a_wide_fan_in_below_the_edge_cap_stops_discovery_at_the_limit() {
    let (mut s, hub) = hub(1_000, true);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, hub, 10 + H);
    assert_eq!(
        r.outcome,
        DurationCone::Fallback(FallbackReason::BackwardLimit)
    );
    assert_eq!(r.moved, [hub]);
    // The hub (once per pass on the heap, counted once) and 100 predecessors.
    assert_eq!(ADMITTED.get(), 101);
    assert_eq!(ENQUEUED.get(), 102);
}

#[test]
fn a_moved_finish_falls_back_both_ways() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let (a, b, side) = (
        add(&mut s, cal, 120),
        add(&mut s, cal, 300),
        add(&mut s, cal, 200),
    );
    link(&mut s, a, b, LinkType::FS, 0);
    let base = s.compute().unwrap();
    let grown = run(&mut s, &base, b, 300 + H);
    assert_eq!(
        grown.outcome,
        DurationCone::Fallback(FallbackReason::FinishMove)
    );
    assert_eq!(grown.moved, [b]);
    s.set_duration(b, 300).unwrap();
    let shrunk = run(&mut s, &base, b, 10);
    assert_eq!(
        shrunk.outcome,
        DurationCone::Fallback(FallbackReason::FinishMove)
    );
    assert_eq!(shrunk.moved, [b]);
    assert!(run_solution(&mut s, b, 10).get(side).is_critical());
    assert_failing_fallback_changes_nothing(&mut s, &base, b, 300 + H, FallbackReason::FinishMove);
}

fn run_solution(s: &mut Schedule, task: TaskId, duration: i64) -> Solution {
    s.set_duration(task, duration).unwrap();
    s.compute().unwrap()
}

#[test]
fn tied_maxima_keep_the_finish_and_stay_incremental() {
    // Two tasks end the project together; shortening one leaves the finish.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let (a, b) = (add(&mut s, cal, 300), add(&mut s, cal, 300));
    for _ in 0..20 {
        add(&mut s, cal, 10);
    }
    let base = s.compute().unwrap();
    assert_eq!(base.get(a).ef, base.get(b).ef);
    let r = run(&mut s, &base, a, 120);
    assert_eq!(r.outcome, DurationCone::Cone, "the tie holds the finish");
    assert_eq!(r.moved, [a]);
    s.set_duration(a, 300).unwrap();
    // Without the tie, the same edit moves the finish.
    let mut alone = Schedule::new();
    let cal = alone.add_calendar(always());
    let a = add(&mut alone, cal, 300);
    let solo = alone.compute().unwrap();
    let r = run(&mut alone, &solo, a, 120);
    assert_eq!(
        r.outcome,
        DurationCone::Fallback(FallbackReason::FinishMove)
    );
}

#[test]
fn an_early_start_past_its_late_start_falls_back_to_the_full_solve() {
    // A stale solution: a predecessor's early start is far past its late start.
    let (mut s, chain) = chain_beside_long(3);
    for _ in 0..40 {
        add(&mut s, CalendarId(0), 10);
    }
    let mut stale = s.compute().unwrap();
    stale.dates[chain[0].0 as usize].es += LONG * 2;
    s.set_duration(chain[2], 10 + H).unwrap();
    let want = s.clone().compute().unwrap();
    let mut got = stale.clone();
    let mut moved = Vec::new();
    let how = s
        .recompute_duration_cone_moved(&mut got, chain[2], &mut moved)
        .unwrap();
    assert_eq!(how, DurationCone::Fallback(FallbackReason::EsAfterLs));
    assert_eq!(LAST.get(), Some(FallbackReason::EsAfterLs));
    assert_eq!(got, want);
    assert_eq!(moved, early_diff(&s, &stale, &want));
    assert_failing_fallback_changes_nothing(
        &mut s,
        &stale,
        chain[2],
        10 + H,
        FallbackReason::EsAfterLs,
    );
}

#[test]
fn a_wrong_length_solution_is_replaced_and_every_task_reported() {
    let (mut s, chain) = chain_beside_long(5);
    s.set_duration(chain[1], 10 + H).unwrap();
    let mut sol = Solution {
        dates: Vec::new(),
        project_finish: 0,
    };
    let mut moved = Vec::new();
    let how = s
        .recompute_duration_cone_moved(&mut sol, chain[1], &mut moved)
        .unwrap();
    assert_eq!(how, DurationCone::Replaced);
    assert_eq!(LAST.get(), None, "a replacement names no fallback");
    let want = s.compute().unwrap();
    assert_eq!(sol, want);
    // No old dates exist: `was_*` are the new dates, in index order.
    let expected: Vec<Moved> = (0..6)
        .map(|i| Moved::was(i, &want.dates[i as usize]))
        .collect();
    assert_eq!(moved, expected, "index order, nothing read as moved");
}

#[test]
fn unknown_and_deleted_roots_are_refused_and_change_nothing() {
    let (mut s, chain) = chain_beside_long(5);
    let base = s.compute().unwrap();
    s.delete_task(chain[2]).unwrap();
    let after = s.compute().unwrap();
    for (sol, task) in [(&base, TaskId(99)), (&after, chain[2])] {
        let (mut got, mut moved) = (sol.clone(), stale(1));
        let err = s.recompute_duration_cone_moved(&mut got, task, &mut moved);
        assert_eq!(err, Err(ScheduleError::UnknownTask(task)));
        assert_eq!((&got, moved.len()), (sol, 0));
    }
}

#[test]
fn a_backward_failure_restores_everything_and_recovery_succeeds() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let id: Vec<TaskId> = (0..3).map(|_| add(&mut s, cal, 100)).collect();
    link(&mut s, id[0], id[1], LinkType::FS, 0);
    link(&mut s, id[1], id[2], LinkType::FS, 0);
    for _ in 0..40 {
        add(&mut s, cal, 10);
    }
    add(&mut s, cal, LONG);
    let base = s.compute().unwrap();
    s.set_duration(id[2], 100 + H).unwrap();
    let want = s.clone().compute().unwrap();
    // Fail the middle task: the root was already written by both passes.
    FAIL_BACKWARD.set(Some(id[1]));
    let (mut sol, mut moved) = (base.clone(), stale(5));
    let got = s.recompute_duration_cone_moved(&mut sol, id[2], &mut moved);
    FAIL_BACKWARD.set(None);
    assert_eq!(got, Err(ScheduleError::Infeasible(id[1])));
    assert_eq!(sol, base);
    assert_eq!(moved, Vec::new());
    s.recompute_duration_cone_moved(&mut sol, id[2], &mut moved)
        .unwrap();
    assert_eq!(sol, want);
    assert_eq!(moved, early_diff(&s, &base, &want));
    assert_eq!(moved.len(), 1, "only the root moved");
}

/// Cone and full compute agree over generated and stress fixtures, for edits
/// that grow, shrink, zero and greatly lengthen a task; both the cone and the
/// fallbacks are exercised (the tallies prove it).
#[test]
fn generated_and_stress_fixtures_match_full_compute() {
    let mut tally = std::collections::HashMap::new();
    let fixtures = (0..6)
        .map(|seed| generate(seed, 300))
        .chain([stress(7, 300).0]);
    for mut s in fixtures {
        let base = s.compute().unwrap();
        for t in (0..300).step_by(11).map(TaskId) {
            let old = s.tasks()[t.0 as usize].duration;
            for d in [old + H, (old - H).max(0), 0, old + 3 * DAY] {
                if s.clone().compute().is_err() {
                    continue;
                }
                let r = run(&mut s, &base, t, d);
                *tally.entry(r.outcome).or_insert(0) += 1;
                s.set_duration(t, old).unwrap();
            }
        }
    }
    let seen = |o| tally.get(&o).copied().unwrap_or(0);
    assert!(seen(DurationCone::Cone) > 20, "{tally:?}");
    assert!(
        seen(DurationCone::Fallback(FallbackReason::FinishMove)) > 10,
        "{tally:?}"
    );
}

// The bounded traversal (gs#58 PR 3): the work queue visits discovered tasks
// only, each once, in rank order. `ENQUEUED` and `ADMITTED` are read after
// `run`, which also checks the result against a full compute.

/// `len` filler tasks and one long task that holds the project finish.
fn add_fillers(s: &mut Schedule, len: usize) {
    for _ in 0..len {
        add(s, CalendarId(0), 10);
    }
    add(s, CalendarId(0), LONG);
}

#[test]
fn a_diamond_joins_two_changed_inputs_and_queues_the_join_once() {
    // a -> {b, c} -> d, beside 41 other tasks: the limit is 5.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let id: Vec<TaskId> = (0..4).map(|_| add(&mut s, cal, 100)).collect();
    for (p, q) in [(0, 1), (0, 2), (1, 3), (2, 3)] {
        link(&mut s, id[p], id[q], LinkType::FS, 0);
    }
    add_fillers(&mut s, 40);
    let base = s.compute().unwrap();
    // Both of d's inputs change: forward queues a, b, c, d (d once, from two
    // sides) and backward queues a.
    let r = run(&mut s, &base, id[0], 100 + H);
    assert_eq!(r.outcome, DurationCone::Cone);
    assert_eq!(r.moved.len(), 4);
    assert_eq!((ADMITTED.get(), ENQUEUED.get()), (4, 5));
    s.set_duration(id[0], 100).unwrap();
    // Backward from d: d, b and c, then a once from two sides.
    let r = run(&mut s, &base, id[3], 100 + H);
    assert_eq!(r.outcome, DurationCone::Cone);
    assert_eq!(r.moved, [id[3]]);
    assert_eq!((ADMITTED.get(), ENQUEUED.get()), (4, 5));
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the comment above names the tasks x, y and j"
)]
fn propagation_stops_where_a_recomputed_pair_is_unchanged() {
    // x and y both feed j and y is later, so growing x leaves j alone and the
    // tasks after j are never discovered.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let [x, y, j, z1, z2]: [TaskId; 5] =
        std::array::from_fn(|i| add(&mut s, cal, [10, 500, 10, 10, 10][i]));
    for (p, q) in [(x, j), (y, j), (j, z1), (z1, z2)] {
        link(&mut s, p, q, LinkType::FS, 0);
    }
    add_fillers(&mut s, 40);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, x, 10 + H);
    assert_eq!(r.outcome, DurationCone::Cone);
    assert_eq!(r.moved, [x]);
    assert_eq!((ADMITTED.get(), ENQUEUED.get()), (2, 3), "x and j only");
}

#[test]
fn a_dense_join_is_queued_once_however_many_inputs_change() {
    // a -> 60 middles -> j, with 637 other tasks: the limit is 70.
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let a = add(&mut s, cal, 100);
    let j = add(&mut s, cal, 100);
    for _ in 0..60 {
        let m = add(&mut s, cal, 100);
        link(&mut s, a, m, LinkType::FS, 0);
        link(&mut s, m, j, LinkType::FS, 0);
    }
    add_fillers(&mut s, 636);
    let base = s.compute().unwrap();
    let r = run(&mut s, &base, a, 100 + H);
    assert_eq!(r.outcome, DurationCone::Cone);
    assert_eq!(r.moved.len(), 62);
    // Forward: a, 60 middles and j (once, not 60 times); backward: a.
    assert_eq!((ADMITTED.get(), ENQUEUED.get()), (62, 63));
}

/// A lattice of 5 layers of 4 tasks, every task linked to every task of the
/// next layer, beside 200 other tasks. `logical[p]` is the lattice position
/// that is added as task `p`, so a permutation puts the ids against the order.
/// Returns the schedule and the id of each lattice position.
fn lattice(logical: &[usize]) -> (Schedule, Vec<TaskId>) {
    let mut s = Schedule::new();
    let cal = s.add_calendar(always());
    let mut id = vec![TaskId(0); 20];
    for &l in logical {
        id[l] = add(&mut s, cal, 100);
    }
    for l in 0..16 {
        for next in (l / 4 + 1) * 4..(l / 4 + 2) * 4 {
            link(&mut s, id[l], id[next], LinkType::FS, 0);
        }
    }
    add_fillers(&mut s, 200);
    (s, id)
}

#[test]
fn permuted_ids_give_the_same_results_as_a_full_compute() {
    let identity: Vec<usize> = (0..20).collect();
    let reversed: Vec<usize> = (0..20).rev().collect();
    let stride: Vec<usize> = (0..20).map(|p| p * 7 % 20).collect();
    for (name, perm) in [
        ("identity", identity),
        ("reversed", reversed),
        ("stride", stride),
    ] {
        let (mut s, id) = lattice(&perm);
        let base = s.compute().unwrap();
        let against = s.dependencies().iter().any(|d| d.pred.0 > d.succ.0);
        assert_eq!(against, name != "identity", "{name}: ids against the order");
        for l in [0, 3, 4, 9, 10, 15, 16, 19] {
            for d in [100 + H, 40, 0] {
                let r = run(&mut s, &base, id[l], d);
                assert_eq!(r.outcome, DurationCone::Cone, "{name}: lattice {l} to {d}");
                s.set_duration(id[l], 100).unwrap();
            }
        }
    }
}
