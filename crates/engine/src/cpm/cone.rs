//! Re-solving the forward cone of one changed task.

#[cfg(test)]
use super::INJECT;
use super::pass::Pass;
use super::{Moved, Solution, TaskDates};
use crate::model::{Schedule, ScheduleError, TaskId};

impl Schedule {
    /// The same call as [`Schedule::recompute_cone_moved`], without returning
    /// the list of tasks whose early dates moved.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] if `task` is unknown or deleted, and otherwise the errors
    /// of [`Schedule::compute`].
    pub fn recompute_cone(
        &mut self,
        solution: &mut Solution,
        task: TaskId,
    ) -> Result<(), ScheduleError> {
        self.cone(solution, task, None)
    }

    /// Bring `solution` up to date after `task`'s own constraint changed
    /// (a start-no-earlier-than set or cleared, as a drag does), touching
    /// only what the change reaches. `solution` must be the result for the
    /// schedule as it was before that change; the answer equals
    /// [`Schedule::compute`] on the changed schedule, date for date.
    /// A change of the data date ([`Schedule::set_data_date`]) is not a cone: it reaches every
    /// unstarted task, so run [`Schedule::compute`] instead.
    ///
    /// * Forward: only `task` and the tasks downstream of it whose
    ///   predecessors' early dates moved are recomputed, in topological
    ///   order. A task none of whose predecessors moved keeps its dates,
    ///   and so does everything it alone could reach.
    /// * Backward: a late date depends on the successors' late dates and on
    ///   the project finish, and a constraint change moves neither unless the
    ///   project finish moves. While it holds still, late dates stand and
    ///   only the float of tasks whose early start moved is redone. When it
    ///   moves, the whole backward pass runs again.
    ///
    /// `moved` is cleared on entry. On success it lists, in topological order,
    /// every task whose early start or early finish changed, with the early
    /// dates it had before ([`Moved`]); changes only to late dates or float are
    /// not reported. A `solution` of the wrong length is replaced by a full
    /// [`Schedule::compute`], and every task is reported in task-index order
    /// with `was_es` and `was_ef` equal to its new dates, as there are no old
    /// ones. On `Err`, `solution` is left as it was and `moved`
    /// is empty. [`Schedule::recompute_cone`] is the same call without this
    /// list.
    ///
    /// In `#[cfg(test)]` builds only, a test seam can omit a cone member or
    /// force a forward-pass failure.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] if `task` is unknown or deleted, and otherwise the errors
    /// of [`Schedule::compute`].
    pub fn recompute_cone_moved(
        &mut self,
        solution: &mut Solution,
        task: TaskId,
        moved: &mut Vec<Moved>,
    ) -> Result<(), ScheduleError> {
        self.cone(solution, task, Some(moved))
    }

    /// The body of [`Schedule::recompute_cone_moved`]. Without a list
    /// (`recompute_cone`) nothing is collected, so a drag pays no allocation
    /// for a report it does not read.
    #[expect(clippy::too_many_lines, reason = "pass and finish update share state")]
    fn cone(
        &mut self,
        solution: &mut Solution,
        task: TaskId,
        mut moved: Option<&mut Vec<Moved>>,
    ) -> Result<(), ScheduleError> {
        if let Some(list) = moved.as_deref_mut() {
            list.clear();
        }
        let n = self.tasks.len();
        if task.0 as usize >= n || self.tasks[task.0 as usize].deleted {
            return Err(ScheduleError::UnknownTask(task));
        }
        self.graph()?;
        if solution.dates.len() != n {
            *solution = self.compute()?;
            if let Some(list) = moved {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "i indexes solution.dates, one entry per task: a slot below the task \
                              count, which TaskId holds in u32 (unchecked at Schedule::add_task)"
                )]
                list.extend(
                    solution
                        .dates
                        .iter()
                        .enumerate()
                        .map(|(i, d)| Moved::unmoved(i as u32, d)),
                );
            }
            return Ok(());
        }
        if self.has_alap() {
            return self.resolve_in_full(solution, moved);
        }
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
        let old_pf = solution.project_finish;
        let dates = &mut solution.dates;

        // Forward pass over `task` and the tasks downstream of it whose
        // predecessors moved. `reached[s]`: a predecessor of s changed its
        // early dates. `undo` holds the old dates of every task changed.
        let mut reached = vec![false; n];
        let mut undo: Vec<(u32, TaskDates)> = Vec::new();
        let mut lost_pf = false;
        let mut new_max = 0;
        let first = g
            .order
            .iter()
            .position(|&s| s == task.0)
            .expect("every task is in the order");
        let mut result = Ok(());
        #[cfg(test)]
        let inject = INJECT.with(std::cell::Cell::get);
        for &si in &g.order[first..] {
            let s = si as usize;
            if si != task.0 && !reached[s] {
                continue;
            }
            #[cfg(test)]
            if Some(TaskId(si)) == inject.skip {
                continue;
            }
            #[cfg(test)]
            if Some(TaskId(si)) == inject.fail {
                result = Err(ScheduleError::Infeasible(TaskId(si)));
                break;
            }
            let forward = pass.forward(si, |p| (dates[p].es, dates[p].ef));
            match forward {
                Ok((es, ef)) => {
                    let old = dates[s];
                    if (es, ef) == (old.es, old.ef) {
                        continue;
                    }
                    undo.push((si, old));
                    dates[s].es = es;
                    dates[s].ef = ef;
                    new_max = new_max.max(ef);
                    lost_pf |= old.ef >= old_pf && ef < old_pf;
                    for &d in pass.succs(s) {
                        reached[pass.deps[d as usize].succ.0 as usize] = true;
                    }
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        let restore = |dates: &mut [TaskDates], undo: &[(u32, TaskDates)]| {
            for &(si, old) in undo {
                dates[si as usize] = old;
            }
        };
        if let Err(e) = result {
            restore(dates, &undo);
            return Err(e);
        }
        if undo.is_empty() {
            return Ok(());
        }

        // The project finish: the latest early finish. It rose to the
        // latest moved finish, or stayed, or (only when a task that ended it
        // finished earlier) needs a scan for the next latest.
        let pf = if new_max >= old_pf {
            new_max
        } else if lost_pf {
            dates.iter().map(|d| d.ef).max().unwrap_or(0)
        } else {
            old_pf
        };

        let anchor = self.anchor(pf);
        let backward = if pf == old_pf {
            // Late dates stand. A moved task past its late start would be an
            // engine bug; let the full pass report it.
            if undo
                .iter()
                .any(|&(si, _)| dates[si as usize].es > dates[si as usize].ls)
            {
                Some(pass.backward_all(anchor, |p| dates[p].es))
            } else {
                for &(si, _) in &undo {
                    let d = &mut dates[si as usize];
                    d.float = pass.float_of(si as usize, d);
                }
                None
            }
        } else {
            Some(pass.backward_all(anchor, |p| dates[p].es))
        };
        if let Some(late) = backward {
            match late {
                Ok((ls, lf)) => {
                    for (i, d) in dates.iter_mut().enumerate() {
                        if pass.tasks[i].deleted {
                            continue;
                        }
                        d.ls = ls[i];
                        d.lf = lf[i];
                        d.float = pass.float_of(i, d);
                    }
                }
                Err(e) => {
                    restore(dates, &undo);
                    return Err(e);
                }
            }
        }
        solution.project_finish = pf;
        if let Some(list) = moved {
            list.extend(undo.iter().map(|(si, old)| Moved::was(*si, old)));
        }
        Ok(())
    }
}
