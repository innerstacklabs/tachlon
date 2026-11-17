//! The duration workload of gs#58: a task whose duration grows by an hour and
//! shrinks back, on the stress fixture, with the project finish fixed.
//!
//! The target is the first task in ascending `TaskId` order whose +60 working
//! minutes leave the finish alone, move a successor's early dates and move a
//! predecessor's late dates. `select` finds it; `PINNED` records what it
//! found, and `Workload::new` fails setup if the two disagree. Shared with
//! `tests/duration_contract.rs`.
#![allow(
    dead_code,
    reason = "each bench and test that includes this file uses part of it"
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{DepId, DurationCone, Minutes, Moved, Schedule, Solution, TaskId, stress};

/// The fixture seed of the duration groups (the other groups use 42).
pub const SEED: u64 = 20_261_020;
/// The size of the duration change, in working minutes.
pub const DELTA: Minutes = 60;
/// The selected target for each size, by `select` on `stress(SEED, n)`.
pub const PINNED: [(usize, u32); 3] = [(10_000, 1), (50_000, 1), (100_000, 4)];

/// Does growing `task` by `DELTA` meet the contract? Restores the duration.
pub fn qualifies(s: &mut Schedule, base: &Solution, task: TaskId) -> bool {
    let i = task.0 as usize;
    let original = s.tasks()[i].duration;
    s.set_duration(task, original + DELTA).expect("task exists");
    let grown = s.compute();
    s.set_duration(task, original).expect("task exists");
    let Ok(grown) = grown else { return false };
    if grown.project_finish != base.project_finish {
        return false;
    }
    let early = |a: &Solution, t: usize| (a.dates[t].es, a.dates[t].ef);
    let late = |a: &Solution, t: usize| (a.dates[t].ls, a.dates[t].lf);
    let (mut succ_moved, mut pred_late) = (false, false);
    for (id, d) in s.dependencies().iter().enumerate() {
        if !s.is_dependency_live(DepId(id as u32)) {
            continue;
        }
        let (p, q) = (d.pred.0 as usize, d.succ.0 as usize);
        if p == i {
            succ_moved |= early(&grown, q) != early(base, q);
        }
        if q == i {
            pred_late |= late(&grown, p) != late(base, p);
        }
    }
    succ_moved && pred_late
}

/// The first task in ascending `TaskId` order that qualifies.
pub fn select(s: &mut Schedule, base: &Solution) -> Option<TaskId> {
    (0..s.tasks().len() as u32)
        .map(TaskId)
        .find(|&t| !s.is_deleted(t) && qualifies(s, base, t))
}

/// Tasks whose early dates differ between two solutions, with their dates in
/// `old`, into `moved`.
pub fn moved_between(old: &Solution, new: &Solution, moved: &mut Vec<Moved>) {
    moved.clear();
    moved.extend(
        (0..new.dates.len())
            .filter(|&i| (old.dates[i].es, old.dates[i].ef) != (new.dates[i].es, new.dates[i].ef))
            .map(|i| Moved {
                task: TaskId(i as u32),
                was_es: old.dates[i].es,
                was_ef: old.dates[i].ef,
            }),
    );
}

/// The fixture, its solution and the pinned target, checked.
pub struct Workload {
    pub schedule: Schedule,
    pub solution: Solution,
    pub target: TaskId,
    pub original: Minutes,
    /// Reused by every `step`.
    pub moved: Vec<Moved>,
}

impl Workload {
    /// Build `stress(SEED, n)` and verify the pinned target against `select`.
    pub fn new(n: usize) -> Workload {
        let (mut schedule, _) = stress(SEED, n);
        let solution = schedule.compute().expect("stress fixture computes");
        let pinned = PINNED.iter().find(|p| p.0 == n).expect("a pinned size");
        let target = TaskId(pinned.1);
        let found = select(&mut schedule, &solution);
        assert_eq!(found, Some(target), "stress({SEED}, {n}): selection moved");
        let original = schedule.tasks()[target.0 as usize].duration;
        Workload {
            schedule,
            solution,
            target,
            original,
            moved: Vec::new(),
        }
    }

    /// Set the target's duration (original + `DELTA` when `longer`) and
    /// recompute in full, reporting what moved. This is the timed unit.
    pub fn step(&mut self, longer: bool) {
        let d = self.original + if longer { DELTA } else { 0 };
        self.schedule
            .set_duration(self.target, d)
            .expect("task exists");
        let new = self.schedule.compute().expect("computes");
        moved_between(&self.solution, &new, &mut self.moved);
        self.solution = new;
    }

    /// The same edit as `step`, brought into the solution with
    /// `Schedule::recompute_duration_cone_moved` (what `Edit::SetDuration`
    /// calls), which fills `moved` itself, and return how it ended. This is
    /// the timed unit of `duration_cone`.
    pub fn step_cone(&mut self, longer: bool) -> DurationCone {
        let d = self.original + if longer { DELTA } else { 0 };
        self.schedule
            .set_duration(self.target, d)
            .expect("task exists");
        self.schedule
            .recompute_duration_cone_moved(&mut self.solution, self.target, &mut self.moved)
            .expect("computes")
    }
}
