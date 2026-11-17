//! Re-solving the cone of one changed duration (gs#58).
//!
//! A duration edit moves early dates downstream of the task and late dates
//! upstream of it. Both are found by running the shared per-task steps,
//! [`Pass::forward`] and [`Pass::backward`], over the tasks whose inputs
//! changed and stopping where a recomputed pair is unchanged.
//!
//! The result equals [`Schedule::compute`] when, before the edit,
//! `solution == compute()` and exactly one live task's duration changed since.
//! Anything else (a stale same-length solution, two unapplied edits, link,
//! calendar or lag-policy edits) must use a full compute.

mod transaction;
mod work;

use super::pass::Pass;
use super::{Moved, Solution, TaskDates};
use crate::calendar::Instant;
use crate::model::{Schedule, ScheduleError, TaskId};
pub use transaction::FallbackReason;
use transaction::{Stop, Transaction};
use work::{Direction, WorkQueue};

/// How a duration edit that returned `Ok` was solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationCone {
    /// The cone's two passes solved the edit.
    Cone,
    /// The cone gave up for this reason and a full compute solved the edit.
    Fallback(FallbackReason),
    /// The solution had the wrong length and a full compute replaced it.
    Replaced,
}

/// The fallback reason of the last call, and the test seams that force a
/// fallback to fail, lower the limits or omit a predecessor. `LAST` is read on
/// the `Err` path, where no [`DurationCone`] is returned. Built for the unit
/// tests and for the `test-hooks` feature (`engine::test_hooks`), which the
/// integration tests enable and a release build never does. Each seam is read
/// once per call, on entry.
#[cfg(any(test, feature = "test-hooks"))]
pub mod seam {
    use super::FallbackReason;
    use crate::model::TaskId;
    use std::cell::Cell;

    pub use super::transaction::{Limits, MAX_INSPECTIONS};

    thread_local! {
        /// The reason of the last call's fallback; `None` when it made none.
        pub static LAST: Cell<Option<FallbackReason>> = const { Cell::new(None) };
        /// Make the full compute of this fallback reason fail.
        pub static FAIL_FALLBACK: Cell<Option<FallbackReason>> = const { Cell::new(None) };
        /// Make the backward step of this task fail.
        pub static FAIL_BACKWARD: Cell<Option<TaskId>> = const { Cell::new(None) };
        /// Replace both limits of a call; `None` keeps the production ones.
        pub static LIMITS: Cell<Option<Limits>> = const { Cell::new(None) };
        /// Leave this task out of the backward pass: it is never queued, as if
        /// the pass had missed a predecessor. The result then differs from a
        /// full compute, which is what a test using this seam must catch.
        pub static OMIT_BACKWARD: Cell<Option<TaskId>> = const { Cell::new(None) };
        /// Distinct queue pushes, both passes, of the last cone attempt.
        pub static ENQUEUED: Cell<usize> = const { Cell::new(0) };
        /// Distinct tasks admitted by the last cone attempt.
        pub static ADMITTED: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn enqueued() {
        ENQUEUED.set(ENQUEUED.get() + 1);
    }
}

impl Schedule {
    /// Bring `solution` up to date after `task`'s duration changed, touching
    /// only what the change reaches. `solution` must be the result for the
    /// schedule as it was before the change, and `task` the only thing that
    /// changed; the answer equals [`Schedule::compute`] date for date. The
    /// SNET counterpart is [`Schedule::recompute_cone_moved`].
    /// A change of the data date ([`Schedule::set_data_date`]) is not a cone: run
    /// [`Schedule::compute`] instead.
    ///
    /// * Forward: `task`, then successors in topological order while a
    ///   recomputed `(es, ef)` differs.
    /// * Backward: `task` always, then predecessors in reverse topological
    ///   order while a recomputed `(ls, lf)` differs.
    /// * Float is redone, after both passes, where `es` or `ls` moved.
    ///
    /// The cone gives up and runs a full compute when the project finish
    /// moves, when it would admit more than `max(1, min(2048, live / 10))`
    /// tasks or inspect more than 16384 dependencies, or when an early start
    /// lands past its late start. A `solution` of the wrong length is replaced
    /// by a full compute.
    ///
    /// On success the value says how the edit was solved ([`DurationCone`]).
    ///
    /// `moved` is cleared on entry. On success it lists, in topological order,
    /// every task whose early start or finish changed, with the early dates it
    /// had before ([`Moved`]; late dates and float are not reported); after a
    /// wrong-length replacement it lists every task in index order, with
    /// `was_es` and `was_ef` equal to the new dates, as there are no old ones.
    /// On `Err`, `solution` is left as it was and `moved`
    /// is empty. An unknown or deleted `task` is [`ScheduleError::UnknownTask`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] if `task` is unknown or deleted, and otherwise the errors
    /// of [`Schedule::compute`].
    pub fn recompute_duration_cone_moved(
        &mut self,
        solution: &mut Solution,
        task: TaskId,
        moved: &mut Vec<Moved>,
    ) -> Result<DurationCone, ScheduleError> {
        moved.clear();
        #[cfg(any(test, feature = "test-hooks"))]
        seam::LAST.set(None);
        let n = self.tasks.len();
        if task.0 as usize >= n || self.tasks[task.0 as usize].deleted {
            return Err(ScheduleError::UnknownTask(task));
        }
        self.graph()?;
        if solution.dates.len() != n {
            *solution = self.compute()?;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "i indexes solution.dates, one entry per task: a slot below the task \
                          count, which TaskId holds in u32 (unchecked at Schedule::add_task)"
            )]
            moved.extend(
                solution
                    .dates
                    .iter()
                    .enumerate()
                    .map(|(i, d)| Moved::unmoved(i as u32, d)),
            );
            return Ok(DurationCone::Replaced);
        }
        let attempt = if self.has_alap() {
            Err(Stop::Fallback(FallbackReason::Alap))
        } else {
            self.duration_cone(solution, task, moved)
        };
        match attempt {
            Ok(()) => Ok(DurationCone::Cone),
            Err(Stop::Error(e)) => Err(e),
            Err(Stop::Fallback(reason)) => {
                #[cfg(any(test, feature = "test-hooks"))]
                seam::LAST.set(Some(reason));
                self.full_fallback(solution, reason, moved)
                    .map(|()| DurationCone::Fallback(reason))
            }
        }
    }

    /// The cone's two passes. On `Err` the journal is already restored.
    fn duration_cone(
        &self,
        solution: &mut Solution,
        task: TaskId,
        moved: &mut Vec<Moved>,
    ) -> Result<(), Stop> {
        let g = self.graph.as_ref().expect("graph() just built it");
        let pass = Pass {
            tasks: &self.tasks,
            deps: &self.deps,
            cals: &self.calendars,
            g,
            lag: self.lag_source(),
            data_date: self.data_date.unwrap_or(0),
            limited: self.limited,
            unstarted_remaining: self.options.unstarted_remaining,
        };
        let mut tx = Transaction::new(g.order.len());
        let dates = &mut solution.dates;
        #[cfg(any(test, feature = "test-hooks"))]
        seam::ENQUEUED.set(0);
        let pf = solution.project_finish;
        let result = passes(&pass, dates, &mut tx, task.0, (pf, self.anchor(pf)));
        #[cfg(any(test, feature = "test-hooks"))]
        seam::ADMITTED.set(tx.admitted());
        match result {
            Ok(changed) => {
                moved.extend(changed);
                Ok(())
            }
            Err(stop) => {
                tx.restore(dates);
                Err(stop)
            }
        }
    }

    /// Solve the edit by a full compute. `solution` is back at its old dates
    /// here; it is replaced, and `moved` filled, only if the compute succeeds.
    fn full_fallback(
        &mut self,
        solution: &mut Solution,
        reason: FallbackReason,
        moved: &mut Vec<Moved>,
    ) -> Result<(), ScheduleError> {
        #[cfg(any(test, feature = "test-hooks"))]
        if seam::FAIL_FALLBACK.get() == Some(reason) {
            return Err(ScheduleError::Infeasible(TaskId(0)));
        }
        let _ = reason;
        self.resolve_in_full(solution, Some(moved))
    }
}

/// Run both passes over the cone of `root`, writing through `tx`. Returns the
/// tasks whose early dates changed, in topological order.
///
/// A task is admitted when it is discovered, before it is queued, so a wide
/// fan-out or fan-in stops at the limit without enqueueing the rest. Each pass
/// walks a [`WorkQueue`]: discovered tasks only, each once, in rank order. The backward pass
/// runs from `anchor` ([`Schedule::anchor`] of the old project finish `old_pf`).
fn passes(
    pass: &Pass<'_>,
    dates: &mut [TaskDates],
    tx: &mut Transaction,
    root: u32,
    (old_pf, anchor): (Instant, Instant),
) -> Result<Vec<Moved>, Stop> {
    let g = pass.g;
    let dep_of = |d: u32| &pass.deps[d as usize];

    // Forward, by ascending rank.
    let mut changed: Vec<Moved> = Vec::new();
    let (mut new_max, mut lost_pf) = (0, false);
    tx.admit(root, FallbackReason::ForwardLimit)?;
    let mut queue = WorkQueue::new(g, Direction::Forward);
    queue.push(root);
    while let Some(si) = queue.pop() {
        let s = si as usize;
        tx.charge(pass.preds(s).len())?;
        let (es, ef) = pass.forward(si, |p| (dates[p].es, dates[p].ef))?;
        let old = dates[s];
        if (es, ef) == (old.es, old.ef) {
            continue;
        }
        tx.write(dates, si, |d| (d.es, d.ef) = (es, ef));
        changed.push(Moved::was(si, &old));
        new_max = new_max.max(ef);
        lost_pf |= old.ef >= old_pf && ef < old_pf;
        tx.charge(pass.succs(s).len())?;
        for &d in pass.succs(s) {
            let q = dep_of(d).succ.0;
            tx.admit(q, FallbackReason::ForwardLimit)?;
            queue.push(q);
        }
    }

    // The latest early finish: the latest changed one, or the old one, or (only
    // when a task that ended it finished earlier) the next latest by a scan.
    let pf = if new_max >= old_pf {
        new_max
    } else if lost_pf {
        dates.iter().map(|d| d.ef).max().unwrap_or(0)
    } else {
        old_pf
    };
    if pf != old_pf {
        return Err(FallbackReason::FinishMove.into());
    }

    // Backward, by descending rank. The root was admitted by the forward pass.
    #[cfg(any(test, feature = "test-hooks"))]
    let (fail, omit) = (seam::FAIL_BACKWARD.get(), seam::OMIT_BACKWARD.get());
    let mut queue = WorkQueue::new(g, Direction::Backward);
    queue.push(root);
    while let Some(si) = queue.pop() {
        let s = si as usize;
        tx.charge(pass.succs(s).len())?;
        #[cfg(any(test, feature = "test-hooks"))]
        if fail == Some(TaskId(si)) {
            return Err(ScheduleError::Infeasible(TaskId(si)).into());
        }
        let late = pass.backward(si, anchor, dates[s].es, |q| (dates[q].ls, dates[q].lf));
        let (ls, lf) = match late {
            Ok(late) => late,
            Err(ScheduleError::InvariantViolated { .. }) => {
                return Err(FallbackReason::EsAfterLs.into());
            }
            Err(e) => return Err(e.into()),
        };
        if (ls, lf) == (dates[s].ls, dates[s].lf) {
            continue;
        }
        tx.write(dates, si, |d| (d.ls, d.lf) = (ls, lf));
        tx.charge(pass.preds(s).len())?;
        for &d in pass.preds(s) {
            let p = dep_of(d).pred.0;
            #[cfg(any(test, feature = "test-hooks"))]
            if omit == Some(TaskId(p)) {
                continue;
            }
            tx.admit(p, FallbackReason::BackwardLimit)?;
            queue.push(p);
        }
    }

    // Float, only now that both passes are final.
    for &(si, old) in tx.journal() {
        let d = &mut dates[si as usize];
        if d.es > d.ls {
            return Err(FallbackReason::EsAfterLs.into());
        }
        if (d.es, d.ef, d.ls, d.lf) != (old.es, old.ef, old.ls, old.lf) {
            d.float = pass.float_of(si as usize, d);
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests;
