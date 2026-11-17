//! The edit chain of the duration-cone equivalence matrix (gs#58 S4).
//!
//! One chain edits a schedule `edits` times with the eight operations of
//! [`Op`], in a shuffled cycle: every group of eight edits takes each
//! operation once, so a chain of at least eight edits cannot miss one. A
//! duration operation goes through `assert_duration_cone_outcome` (the cone
//! result and `moved` equal a full compute); an SNET operation goes through
//! `recompute_cone_moved` and the same comparison. The chain is carried
//! forward by the incremental paths alone, so a wrong date that survives one
//! step feeds the next.
//!
//! [`Seen`] tallies which cases the chain reached, counting only the duration
//! edits the cone itself solved (a fallback returns the full compute's answer
//! and says nothing about the cone's passes).

use super::check_invariants;
use super::equiv::{assert_duration_cone_outcome, early_diff, moved_at};
use engine::{DAY, DurationCone, FallbackReason, Rng, Schedule, Solution, TaskDates, TaskId};

/// One edit of the cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Duration to zero: the task becomes a milestone.
    Zero,
    /// Shorter by up to two hours, not below zero (decrease toward zero).
    Shorten,
    /// Exactly one minute longer; a task zeroed earlier gets its length back.
    Lengthen,
    /// Exactly 60 minutes longer.
    Hour,
    /// A random new length from zero to two days.
    Redraw,
    /// The same length.
    Keep,
    /// An SNET constraint set or moved.
    SetSnet,
    /// An SNET constraint cleared.
    ClearSnet,
}

pub const CYCLE: [Op; 8] = [
    Op::Zero,
    Op::Shorten,
    Op::Lengthen,
    Op::Hour,
    Op::Redraw,
    Op::Keep,
    Op::SetSnet,
    Op::ClearSnet,
];

/// What a chain reached. The `cone`-based counters count duration edits that
/// the cone solved.
#[derive(Default, Debug, Clone)]
pub struct Seen {
    /// Operations that found a task and ran.
    pub edits: usize,
    /// Edits that both paths refused (and that were rolled back).
    pub refused: usize,
    /// SNET operations whose result equalled a full compute.
    pub snet: usize,
    /// Duration edits solved by the cone's two passes.
    pub cone: usize,
    /// Duration edits solved by a full compute after the project finish moved.
    pub finish_moved: usize,
    /// Duration edits solved by a full compute for any other reason.
    pub other_fallback: usize,
    /// Cone edits that lengthened the task.
    pub grew: usize,
    /// Cone edits that shortened the task.
    pub shrank: usize,
    /// Cone edits that made a task a milestone.
    pub to_milestone: usize,
    /// Cone edits that gave a milestone a length.
    pub from_milestone: usize,
    /// Cone edits that changed the late dates of a task other than the edited
    /// one and none of its early dates: the backward pass alone moved it.
    pub backward_only: usize,
    /// Cone edits where a task that moved had a successor that did not: the
    /// forward pass stopped there.
    pub stopped: usize,
}

impl Seen {
    pub fn add(&mut self, o: &Seen) {
        self.edits += o.edits;
        self.refused += o.refused;
        self.snet += o.snet;
        self.cone += o.cone;
        self.finish_moved += o.finish_moved;
        self.other_fallback += o.other_fallback;
        self.grew += o.grew;
        self.shrank += o.shrank;
        self.to_milestone += o.to_milestone;
        self.from_milestone += o.from_milestone;
        self.backward_only += o.backward_only;
        self.stopped += o.stopped;
    }
}

/// The zero a deleted task's slot reads.
const ZERO: TaskDates = TaskDates {
    es: 0,
    ef: 0,
    ls: 0,
    lf: 0,
    float: 0,
    finished: false,
};

fn shuffle(ops: &mut [Op; 8], rng: &mut Rng) {
    for i in (1..ops.len()).rev() {
        ops.swap(i, rng.range(0, i as i64) as usize);
    }
}

/// A live task, from a random start. `with_snet` wants one carrying an SNET
/// constraint, and gets `None` when there is none.
fn pick(s: &Schedule, rng: &mut Rng, with_snet: bool) -> Option<TaskId> {
    let n = s.tasks().len();
    let start = rng.range(0, n as i64 - 1) as usize;
    (0..n)
        .map(|i| (start + i) % n)
        .map(|i| TaskId(i as u32))
        .find(|&t| !s.is_deleted(t) && (!with_snet || s.tasks()[t.0 as usize].snet.is_some()))
}

/// Edit the schedule `edits` times and check every step against a full
/// compute. Panics, naming `label` and the step, on the first difference.
pub fn run(s: &mut Schedule, rng: &mut Rng, edits: usize, label: &str) -> Seen {
    let mut sol = s.compute().expect("the base schedule computes");
    let mut seen = Seen::default();
    let mut ops = CYCLE;
    let mut zeroed: Option<TaskId> = None;
    for step in 0..edits {
        if step % ops.len() == 0 {
            shuffle(&mut ops, rng);
        }
        let op = ops[step % ops.len()];
        let ctx = format!("{label}, step {step} ({op:?})");
        match op {
            Op::SetSnet | Op::ClearSnet => {
                let Some(task) = pick(s, rng, op == Op::ClearSnet) else {
                    continue;
                };
                let new = match op {
                    Op::ClearSnet => None,
                    _ if rng.chance(0.5) => {
                        let es = sol.get(task).es;
                        Some((es + rng.range(-5 * DAY, 5 * DAY)).max(0))
                    }
                    _ => Some(rng.range(0, sol.project_finish + 14 * DAY)),
                };
                seen.edits += 1;
                if snet_step(s, &mut sol, task, new, &ctx) {
                    seen.snet += 1;
                } else {
                    seen.refused += 1;
                }
            }
            _ => {
                let task = match (op, zeroed) {
                    (Op::Lengthen, Some(t)) if !s.is_deleted(t) => t,
                    _ => match pick(s, rng, false) {
                        Some(t) => t,
                        None => continue,
                    },
                };
                let old = s.tasks()[task.0 as usize].duration;
                let new = match op {
                    Op::Zero => 0,
                    Op::Shorten => (old - rng.range(0, 120)).max(0),
                    Op::Lengthen => old + 1,
                    Op::Hour => old + 60,
                    Op::Redraw => rng.range(0, 2 * DAY),
                    _ => old,
                };
                match op {
                    Op::Zero => zeroed = Some(task),
                    Op::Lengthen => zeroed = None,
                    _ => {}
                }
                seen.edits += 1;
                let before = sol.clone();
                let how = assert_duration_cone_outcome(s, &mut sol, task, new, &ctx);
                tally(s, &mut seen, how, task, (old, new), &before, &sol);
            }
        }
    }
    // `check_invariants` walks every slot as a live task, so a schedule with
    // deleted tasks skips it; each step already equalled a full compute.
    if !s.tasks().iter().any(|t| t.deleted) {
        check_invariants(s, &sol);
    }
    for (t, date) in sol.dates.iter().enumerate().take(s.tasks().len()) {
        if s.is_deleted(TaskId(t as u32)) {
            assert_eq!(*date, ZERO, "{label}: deleted slot {t} is not zero");
        }
    }
    seen
}

/// Set `task`'s SNET and bring `sol` up to date by the SNET cone; the result
/// and its `moved` list must equal a full compute, and a refused edit (both
/// say infeasible) is rolled back and leaves `sol` alone. Returns whether the
/// edit was applied.
fn snet_step(
    s: &mut Schedule,
    sol: &mut Solution,
    task: TaskId,
    new: Option<i64>,
    ctx: &str,
) -> bool {
    let before = s.tasks()[task.0 as usize].snet;
    s.set_snet(task, new).expect("task exists");
    let ctx = format!("{ctx}: {task} snet {before:?} -> {new:?}");
    let want = s.clone().compute();
    let old = sol.clone();
    let mut moved = vec![moved_at(u32::MAX)];
    let got = s.recompute_cone_moved(sol, task, &mut moved);
    match (want, got) {
        (Ok(want), Ok(())) => {
            assert_eq!(*sol, want, "{ctx}");
            let order = s.topological_order().expect("computed");
            assert_eq!(moved, early_diff(&order, &old, &want), "{ctx}: moved");
            true
        }
        (Err(want), Err(got)) => {
            assert_eq!(got, want, "{ctx}: refused differently");
            assert_eq!(*sol, old, "{ctx}: a refused edit changed the solution");
            assert!(moved.is_empty(), "{ctx}: refused edit reported moves");
            s.set_snet(task, before).expect("task exists");
            false
        }
        (want, got) => panic!("{ctx}: full {want:?}, cone {got:?}"),
    }
}

/// Count what one applied duration edit reached.
fn tally(
    s: &Schedule,
    seen: &mut Seen,
    how: Option<DurationCone>,
    task: TaskId,
    (old, new): (i64, i64),
    before: &Solution,
    after: &Solution,
) {
    match how {
        None => seen.refused += 1,
        Some(DurationCone::Fallback(FallbackReason::FinishMove)) => seen.finish_moved += 1,
        Some(DurationCone::Cone) => {
            seen.cone += 1;
            seen.grew += (new > old) as usize;
            seen.shrank += (new < old) as usize;
            seen.to_milestone += (old > 0 && new == 0) as usize;
            seen.from_milestone += (old == 0 && new > 0) as usize;
            let early = |s: &Solution, i: usize| (s.dates[i].es, s.dates[i].ef);
            let late = |s: &Solution, i: usize| (s.dates[i].ls, s.dates[i].lf);
            let moved = |i: usize| early(before, i) != early(after, i);
            let n = after.dates.len();
            seen.backward_only += (0..n)
                .any(|i| i != task.0 as usize && !moved(i) && late(before, i) != late(after, i))
                as usize;
            // A dependency in force from a task that moved to one that did not.
            seen.stopped += s
                .live_dependencies()
                .any(|(_, d)| moved(d.pred.0 as usize) && !moved(d.succ.0 as usize))
                as usize;
        }
        Some(_) => seen.other_fallback += 1,
    }
}
