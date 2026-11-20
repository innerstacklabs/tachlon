//! Free float: how long a task can run late before the very next task has to wait.

use super::pass::Pass;
use super::{Solution, TaskDates};
use crate::calendar::{Instant, Minutes};
use crate::model::{Graph, Schedule, ScheduleError, TaskId};
use crate::topo;

impl Schedule {
    /// The free float of every task of `solution`, indexed by [`TaskId`]: the working minutes of
    /// the task's own calendar it could be delayed before any successor's early date moves, read
    /// from its early start (from its early finish once started, as for total float, decision
    /// 0008). A task with no successor is measured to the computed project finish; a finished task
    /// and a deleted slot have 0, and it is never negative.
    ///
    /// It is the backward step of [`Schedule::compute`], run with each successor's early dates in
    /// place of its late dates (the step an as-late-as-possible task takes), so the four link
    /// rules and the lag calendars are the ones that place the early dates. The textbook value:
    /// the task's own late limits and pins and the must-finish-by date do not shorten it; they act
    /// on late dates and total float, so a missed limit shows there as a negative number.
    ///
    /// `solution` must be the result of [`Schedule::compute`] for this schedule as it is now.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] when `solution` has another task count, and the
    /// error of the pass otherwise ([`ScheduleError::Cycle`], [`ScheduleError::Infeasible`],
    /// [`ScheduleError::InvariantViolated`]).
    pub fn free_floats(&self, solution: &Solution) -> Result<Vec<Minutes>, ScheduleError> {
        self.check_free_inputs(solution)?;
        let built;
        let graph = if let Some(graph) = &self.graph {
            graph
        } else {
            built = topo::build(&self.tasks, &self.deps, &self.removed)?;
            &built
        };
        let pass = self.free_pass(graph);
        (0..self.tasks.len())
            .map(|slot| Self::free_float_at(&pass, solution, slot))
            .collect()
    }

    /// The free float of `task` alone: the value [`Schedule::free_floats`] gives for it, worked from
    /// its own successors, so a caller that needs a few tasks does not run the whole schedule. It
    /// never builds the dependency graph: [`Schedule::compute`] keeps the one it built.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for a `task` past the schedule, when `solution` has
    /// another task count, and when the schedule holds no graph (it changed since `solution` was
    /// computed); otherwise the error of the pass, as [`Schedule::free_floats`].
    pub fn free_float(&self, solution: &Solution, task: TaskId) -> Result<Minutes, ScheduleError> {
        self.check_free_inputs(solution)?;
        let slot = task.0 as usize;
        match &self.graph {
            Some(graph) if slot < self.tasks.len() => {
                Self::free_float_at(&self.free_pass(graph), solution, slot)
            }
            _ => Err(ScheduleError::UnknownTask(task)),
        }
    }

    fn check_free_inputs(&self, solution: &Solution) -> Result<(), ScheduleError> {
        let known = solution.dates.len();
        if known == self.tasks.len() {
            return Ok(());
        }
        let known = u32::try_from(known).unwrap_or(u32::MAX);
        Err(ScheduleError::UnknownTask(TaskId(known)))
    }

    fn free_pass<'a>(&'a self, graph: &'a Graph) -> Pass<'a> {
        Pass {
            tasks: &self.tasks,
            deps: &self.deps,
            cals: &self.calendars,
            g: graph,
            lag: self.lag_source(),
            data_date: self.data_date.unwrap_or(0),
            limited: self.limited,
            unstarted_remaining: self.options.unstarted_remaining,
        }
    }

    fn free_float_at(
        pass: &Pass<'_>,
        solution: &Solution,
        slot: usize,
    ) -> Result<Minutes, ScheduleError> {
        if pass.tasks[slot].deleted {
            return Ok(0);
        }
        let dates = &solution.dates;
        // Where a successor's work starts: a started task's early start is its actual start, but
        // what its predecessors wait for is its remaining work, which ends at its early finish.
        let work_start = |succ: usize| -> Instant {
            let (task, early) = (&pass.tasks[succ], &dates[succ]);
            let cal = pass.cal_of(succ);
            match (task.actual_start, task.work(pass.unstarted_remaining)) {
                (Some(_), work) if work > 0 => cal.nth(cal.count(early.ef) - work),
                (Some(_), _) => early.ef,
                _ => early.es,
            }
        };
        let early = &dates[slot];
        let id = u32::try_from(slot).unwrap_or(u32::MAX);
        let (ls, lf) = pass.backward_with(
            id,
            solution.project_finish,
            early.es,
            |succ| (work_start(succ), dates[succ].ef),
            false,
        )?;
        Ok(pass.float_of(slot, &TaskDates { ls, lf, ..*early }).max(0))
    }
}
