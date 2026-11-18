//! Driving predecessors, driven successors and the longest path.

use super::Solution;
use super::admissible::Admissible;
use super::pass::finish_floor;
use crate::calendar::{Instant, Minutes};
use crate::model::{LinkType, Schedule, TaskId};

/// Maximum difference between a dependency bound and the successor's early start.
const DRIVING_TOLERANCE: Minutes = 1;

impl Schedule {
    // Keep this in step with the dependency cases in `Pass::forward`.
    fn dependency_start(&self, sol: &Solution, di: usize) -> Option<Instant> {
        let d = &self.deps[di];
        let dates = sol.get(d.pred);
        let cal_l = self.lag_source().cal(&self.calendars, &self.tasks, di, d);
        let succ = &self.tasks[d.succ.0 as usize];
        let work = match succ.actual_start {
            Some(_) => succ.duration,
            None => succ.work(self.options.unstarted_remaining),
        };
        let a = Admissible::new(self.calendar_of(d.succ), work).kind(succ.limit.milestone);
        let finish_bound = |bound: Instant| a.earliest(0, bound.max(0), |t| a.fin(t) >= bound);
        match d.kind {
            LinkType::FS => Some(a.next(cal_l.add(dates.ef, d.lag).max(0))),
            LinkType::SS => Some(a.next(cal_l.add(dates.es, d.lag).max(0))),
            LinkType::FF => finish_bound(cal_l.add(dates.ef, d.lag)),
            LinkType::SF => finish_bound(finish_floor(
                self.calendar_of(d.succ),
                cal_l.add(dates.es, d.lag),
            )),
        }
    }

    /// Whether `d` alone sets its successor's early start (within
    /// [`DRIVING_TOLERANCE`]). A `sol` that is not this schedule's solution
    /// drives nothing.
    fn drives(&self, sol: &Solution, di: usize) -> bool {
        let d = &self.deps[di];
        if sol.dates.len() != self.tasks.len() {
            return false;
        }
        let es = sol.get(d.succ).es;
        self.dependency_start(sol, di).is_some_and(|start| {
            start <= es && start.abs_diff(es) <= DRIVING_TOLERANCE.unsigned_abs()
        })
    }

    /// The predecessors whose dependencies set `task`'s early start.
    #[must_use]
    pub fn driving_predecessors(&self, sol: &Solution, task: TaskId) -> Vec<TaskId> {
        let i = task.0 as usize;
        if i >= self.tasks.len() {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Some(g) = &self.graph {
            for &di in &g.pred_deps[g.pred_start[i] as usize..g.pred_start[i + 1] as usize] {
                let d = &self.deps[di as usize];
                if self.drives(sol, di as usize) {
                    result.push(d.pred);
                }
            }
        } else {
            for (di, d) in self
                .deps
                .iter()
                .enumerate()
                .filter(|(i, d)| d.succ == task && self.live_at(*i))
            {
                if self.drives(sol, di) {
                    result.push(d.pred);
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    /// The successors whose early starts are set by dependencies from `task`.
    #[must_use]
    pub fn driven_successors(&self, sol: &Solution, task: TaskId) -> Vec<TaskId> {
        let i = task.0 as usize;
        if i >= self.tasks.len() {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Some(g) = &self.graph {
            for &di in &g.succ_deps[g.succ_start[i] as usize..g.succ_start[i + 1] as usize] {
                let d = &self.deps[di as usize];
                if self.drives(sol, di as usize) {
                    result.push(d.succ);
                }
            }
        } else {
            for (di, d) in self
                .deps
                .iter()
                .enumerate()
                .filter(|(i, d)| d.pred == task && self.live_at(*i))
            {
                if self.drives(sol, di) {
                    result.push(d.succ);
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    /// The tasks on the longest path, traced backward from every task that
    /// finishes at the project finish. A driving link is exactly what
    /// `Schedule::drives` in `crates/engine/src/cpm/driving.rs` decides: the
    /// dependency alone sets the successor's early start within
    /// `DRIVING_TOLERANCE`. A start-no-earlier-than date or a different
    /// driver that sets a later start makes the link non-driving. A link
    /// whose successor start is snapped to the next working time of the
    /// successor's calendar still drives.
    ///
    /// The result is a subset of tasks and may differ from the zero-float
    /// (critical) set. A zero-float task can fail to drive anything: in
    /// `longest_path_differs_from_zero_float_with_snet` (`hand_solved.rs`) X
    /// has zero float, but Y's start-no-earlier-than date, not the
    /// non-working time between them, sets Y's start, so X does not drive Y.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "i indexes the tasks (sol.dates matches their length): a slot below the task \
                  count, which TaskId holds in u32 (unchecked at Schedule::add_task)"
    )]
    #[must_use]
    pub fn longest_path(&self, sol: &Solution) -> Vec<TaskId> {
        if sol.dates.len() != self.tasks.len() {
            return Vec::new();
        }
        let mut visited = vec![false; self.tasks.len()];
        let mut stack = Vec::new();
        for (i, dates) in sol.dates.iter().enumerate() {
            if dates.ef == sol.project_finish && !self.tasks[i].deleted {
                visited[i] = true;
                stack.push(TaskId(i as u32));
            }
        }
        while let Some(task) = stack.pop() {
            for pred in self.driving_predecessors(sol, task) {
                let i = pred.0 as usize;
                if !visited[i] {
                    visited[i] = true;
                    stack.push(pred);
                }
            }
        }
        visited
            .into_iter()
            .enumerate()
            .filter_map(|(i, seen)| seen.then_some(TaskId(i as u32)))
            .collect()
    }
}
