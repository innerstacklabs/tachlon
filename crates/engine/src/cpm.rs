//! Critical path method by search (plan, "Engine (M1)").
//!
//! [`Calendar::add`](crate::calendar::Calendar::add) collapses non-working gaps, so it has no inverse. The
//! passes never invert it (no negated lag, no subtracted duration). Every
//! bound is found by binary search over a task's admissible start instants,
//! testing the ORIGINAL dependency inequality written with forward operations
//! only. This is sound because `fin(T, s)` and `add(cal, t, w)` are monotone
//! non-decreasing in `s` and `t`, so every predicate is monotone in the
//! searched start.
//!
//! The backward pass does read each inequality backwards once, as a guess at
//! the latest start, and tests the guess and its admissible neighbour with the
//! forward predicates. The guess only narrows the search (a wrong guess costs
//! a full search, never a different date).

mod admissible;
mod cone;
mod driving;
pub(crate) mod duration;
mod free;
mod moved;
mod pass;

pub use admissible::Admissible;
pub use moved::Moved;

use crate::calendar::{Instant, Minutes};
use crate::model::{Schedule, ScheduleError, TaskId};
use pass::Pass;

/// Dates of one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDates {
    /// Early start.
    pub es: Instant,
    /// Early finish, `fin(T, es)`.
    pub ef: Instant,
    /// Late start; before the origin (negative) when a late limit cannot be met even from there.
    pub ls: Instant,
    /// Late finish, `fin(T, ls)`.
    pub lf: Instant,
    /// Total float: working minutes of the task's calendar in `[es, ls)`, negative when a late
    /// limit ([`Limit`](crate::Limit)) puts `ls` before `es`.
    pub float: Minutes,
    /// The task has an actual finish ([`Task::actual_finish`](crate::Task::actual_finish)). Its
    /// float is 0 (decision 0008), but finished work is not critical.
    pub finished: bool,
}

impl TaskDates {
    /// Critical = float at or below zero, and not finished: the remaining work decides the critical
    /// path (decision 0008, point 3a).
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.float <= 0 && !self.finished
    }
}

/// Result of [`Schedule::compute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solution {
    /// Dates per task, indexed by [`TaskId`].
    pub dates: Vec<TaskDates>,
    /// Project finish: the latest early finish (the origin if there are no tasks).
    pub project_finish: Instant,
}

impl Solution {
    /// Dates of one task.
    #[must_use]
    pub fn get(&self, task: TaskId) -> &TaskDates {
        &self.dates[task.0 as usize]
    }
}

/// Test-only seam for omitting a cone member or forcing its forward pass to fail.
#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct Inject {
    skip: Option<TaskId>,
    fail: Option<TaskId>,
}

#[cfg(test)]
thread_local! {
    static INJECT: std::cell::Cell<Inject> = const {
        std::cell::Cell::new(Inject { skip: None, fail: None })
    };
}

impl Schedule {
    /// Some live task is as late as possible ([`Limit::alap`](crate::Limit::alap)): its place
    /// reads its successors' early dates, which a cone does not re-derive, so every cone takes a
    /// full compute instead.
    fn has_alap(&self) -> bool {
        self.limited && self.tasks.iter().any(|t| t.limit.alap && !t.deleted)
    }

    /// Replace `solution` with a full [`Schedule::compute`], and list in `moved`, in topological
    /// order, the tasks whose early dates differ, with the dates they had.
    fn resolve_in_full(
        &mut self,
        solution: &mut Solution,
        moved: Option<&mut Vec<Moved>>,
    ) -> Result<(), ScheduleError> {
        let fresh = self.compute()?;
        let g = self.graph.as_ref().expect("compute built the graph");
        let early = |s: &Solution, i: usize| (s.dates[i].es, s.dates[i].ef);
        if let Some(list) = moved {
            list.extend(
                g.order
                    .iter()
                    .filter(|&&i| early(solution, i as usize) != early(&fresh, i as usize))
                    .map(|&i| Moved::was(i, &solution.dates[i as usize])),
            );
        }
        *solution = fresh;
        Ok(())
    }

    /// Forward and backward pass over the cached topological order.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::Cycle`] if the dependencies contain a loop,
    /// [`ScheduleError::Infeasible`] if a task has no admissible start, and
    /// [`ScheduleError::InvariantViolated`] if an early start lands past its late start with no
    /// [`Limit`](crate::Limit) in the schedule (with one, the float is negative instead).
    #[expect(
        clippy::missing_panics_doc,
        reason = "the expect reads the graph built just above"
    )]
    pub fn compute(&mut self) -> Result<Solution, ScheduleError> {
        self.graph()?;
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
        let n = self.tasks.len();

        let mut es = vec![0 as Instant; n];
        let mut ef = vec![0 as Instant; n];
        for &si in &g.order {
            let s = si as usize;
            (es[s], ef[s]) = pass.forward(si, |p| (es[p], ef[p]))?;
        }
        let pf = ef.iter().copied().max().unwrap_or(0);
        pass.alap(pf, &mut es, &mut ef)?;
        let (ls, lf) = pass.backward_all(self.anchor(pf), |p| es[p])?;

        // A deleted task's slot stays all zero; it is not worth a calendar walk.
        let mut dates: Vec<TaskDates> = (0..n)
            .map(|i| TaskDates {
                es: es[i],
                ef: ef[i],
                ls: ls[i],
                lf: lf[i],
                float: 0,
                finished: false,
            })
            .collect();
        for (i, d) in dates.iter_mut().enumerate() {
            if !self.tasks[i].deleted {
                d.float = pass.float_of(i, d);
                d.finished = self.tasks[i].actual_finish.is_some();
            }
        }
        Ok(Solution {
            dates,
            project_finish: pf,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::admissible::late_start;
    use super::*;
    use crate::calendar::{Calendar, DAY, DayPeriod, WEEK};
    use crate::model::DepId;

    fn with_inject<R>(inject: Inject, f: impl FnOnce() -> R) -> R {
        struct ResetInject;

        impl Drop for ResetInject {
            fn drop(&mut self) {
                INJECT.with(|cell| cell.set(Inject::default()));
            }
        }

        INJECT.with(|cell| cell.set(inject));
        let _reset = ResetInject;
        f()
    }

    /// Every minute working, so every instant is an admissible start.
    fn always() -> Calendar {
        let day: Vec<DayPeriod> = vec![(0, 1440)];
        Calendar::new(std::array::from_fn(|_| day.clone()), &[]).unwrap()
    }

    #[test]
    fn late_start_answer_does_not_depend_on_the_guess() {
        let cal = always();
        let a = Admissible::new(&cal, 60);
        let pred = |x: Instant| (x <= 100).then(|| a.fin(x));
        // Exact, too low (pred holds at its neighbour too), too high, at the
        // origin, past `hi`.
        let late = |hi, guess| late_start(&a, TaskId(3), 0, hi, guess, pred, || None);
        for guess in [100, 50, 99, 101, 500, 0, 2000] {
            let (start, finish) = late(1000, guess).unwrap();
            assert_eq!(start, 100, "guess {guess}");
            assert_eq!(finish, a.fin(start), "guess {guess}");
        }
        // The answer at the top of the domain.
        assert_eq!(late(80, 40), Ok((80, a.fin(80))));
    }

    #[test]
    fn late_start_right_guess_reuses_the_predicate_finish() {
        let cal = always();
        let a = Admissible::new(&cal, 60);
        let calls = std::cell::Cell::new(0);
        let pred = |x: Instant| {
            calls.set(calls.get() + 1);
            (x <= 100).then(|| a.fin(x))
        };

        assert_eq!(
            late_start(&a, TaskId(3), 0, 1000, 100, pred, || None),
            Ok((100, a.fin(100)))
        );
        assert_eq!(calls.get(), 2);

        calls.set(0);
        assert_eq!(
            late_start(&a, TaskId(3), 0, 100, 100, pred, || None),
            Ok((100, a.fin(100)))
        );
        assert_eq!(calls.get(), 1);
    }

    /// `earliest` bounds its probes, not its result (review note on #6): with
    /// `probe_hi` on a weekend the answer is the next Monday's first working
    /// minute, past `probe_hi`, and it is still the earliest admissible start.
    #[test]
    fn earliest_result_can_pass_probe_hi() {
        let day: Vec<DayPeriod> = vec![(480, 1020)];
        let mut week: [Vec<DayPeriod>; 7] = std::array::from_fn(|_| day.clone());
        week[5].clear();
        week[6].clear();
        let cal = Calendar::new(week, &[]).unwrap();
        let saturday_noon = 5 * DAY + 720;
        let monday_8 = WEEK + 480;
        let friday_17 = 4 * DAY + 1020;
        let a = Admissible::new(&cal, 60);
        let got = a.earliest(saturday_noon, saturday_noon, |_| true);
        assert_eq!(got, Some(monday_8));
        // The forward pass's shape, a finish bound in non-working time: the
        // earliest start reaching it is Friday 16:01 (finish Monday 08:01).
        let got = a.earliest(0, saturday_noon, |t| a.fin(t) >= saturday_noon);
        assert_eq!(got, Some(friday_17 - 59));
        // A finish bound inside working time stays bounded by it.
        let got = a.earliest(0, friday_17, |t| a.fin(t) >= friday_17);
        assert_eq!(got, Some(friday_17 - 60));
    }

    /// An early start that fails the backward constraints is an engine bug,
    /// whether or not some other start satisfies them (review note on #6:
    /// finding none used to be reported as `Infeasible`).
    #[test]
    fn late_start_reports_es_after_ls_as_an_invariant_violation() {
        let cal = always();
        let a = Admissible::new(&cal, 60);
        let blame = || Some(DepId(7));
        let violated = |ls| {
            Err(ScheduleError::InvariantViolated {
                task: TaskId(3),
                dep: Some(DepId(7)),
                es: 200,
                ls,
            })
        };
        let some = late_start(
            &a,
            TaskId(3),
            200,
            1000,
            150,
            |x| (x <= 100).then(|| a.fin(x)),
            blame,
        );
        assert_eq!(some, violated(Some(100)));
        let none = late_start(&a, TaskId(3), 200, 1000, 150, |_| None, blame);
        assert_eq!(none, violated(None));
    }

    #[test]
    fn late_start_preserves_the_error_payload_and_recovers_from_a_wrong_guess() {
        let cal = always();
        let a = Admissible::new(&cal, 60);
        let pred = |x: Instant| (x <= 100).then(|| a.fin(x));

        assert_eq!(
            late_start(&a, TaskId(3), 200, 1000, 500, pred, || Some(DepId(7))),
            Err(ScheduleError::InvariantViolated {
                task: TaskId(3),
                dep: Some(DepId(7)),
                es: 200,
                ls: Some(100),
            })
        );
        assert_eq!(
            late_start(&a, TaskId(3), 0, 1000, 500, pred, || None),
            Ok((100, a.fin(100)))
        );
    }

    /// The break for the equivalence tests in `tests/cone.rs`: with any one
    /// task whose early dates the edit moves left out of the cone, the result
    /// differs from a full recompute. A comparison that could not see a
    /// missing member would let a wrong cone through.
    #[test]
    fn a_cone_missing_one_member_differs_from_the_full_recompute() {
        use crate::generate::generate;
        let mut caught = 0;
        for seed in 0..6 {
            let base = generate(seed, 400);
            let mut s = base.clone();
            let before = s.compute().unwrap();
            for t in (0..400).step_by(37) {
                let task = TaskId(t);
                let mut edited = base.clone();
                edited
                    .set_snet(task, Some(before.get(task).es + 6 * DAY))
                    .unwrap();
                let want = edited.compute().unwrap();
                // The downstream members the edit moved.
                #[expect(clippy::cast_possible_truncation, reason = "i is below 400")]
                let moved: Vec<TaskId> = (0..400)
                    .filter(|&i| {
                        let (a, b) = (&before.dates[i], &want.dates[i]);
                        i != t as usize && (a.es, a.ef) != (b.es, b.ef)
                    })
                    .map(|i| TaskId(i as u32))
                    .collect();
                // The whole cone matches.
                let mut sol = before.clone();
                edited.recompute_cone(&mut sol, task).unwrap();
                assert_eq!(sol, want, "seed {seed}, {task}");
                for &skipped in &moved {
                    let mut sol = before.clone();
                    // Wrong dates, or an inconsistent schedule the backward
                    // pass refuses (the solution is then left as it was).
                    let got = with_inject(
                        Inject {
                            skip: Some(skipped),
                            ..Inject::default()
                        },
                        || edited.recompute_cone(&mut sol, task),
                    );
                    if got.is_ok() {
                        assert_ne!(sol, want, "seed {seed}, {task}, skipping {skipped}");
                    } else {
                        assert_eq!(sol, before, "seed {seed}, {task}, skipping {skipped}");
                    }
                    caught += 1;
                }
            }
        }
        assert!(caught > 100, "only {caught} missing members tried");
    }

    /// No edit the sweeps make fails the forward step (`Admissible::earliest`
    /// probes at `max(lo, bound)`, where its predicate holds), so they never
    /// reach the rollback of a forward error; the seam forces it. Deleting
    /// `restore(dates, &undo)` from the forward `Err` branch fails this test.
    #[test]
    fn a_forward_error_rolls_back_every_task_the_cone_moved() {
        use crate::generate::generate;
        let mut exercised = 0;
        for seed in 0..6 {
            let base = generate(seed, 400);
            let mut s = base.clone();
            let before = s.compute().unwrap();
            for t in (0..400).step_by(37) {
                let task = TaskId(t);
                let mut edited = base.clone();
                edited
                    .set_snet(task, Some(before.get(task).es + 6 * DAY))
                    .unwrap();
                let want = edited.compute().unwrap();
                #[expect(clippy::cast_possible_truncation, reason = "i is below 400")]
                let moved: Vec<TaskId> = (0..400)
                    .filter(|&i| {
                        let (a, b) = (&before.dates[i], &want.dates[i]);
                        i != t as usize && (a.es, a.ef) != (b.es, b.ef)
                    })
                    .map(|i| TaskId(i as u32))
                    .collect();
                if moved.len() < 2 {
                    continue;
                }
                let fail = edited
                    .graph
                    .as_ref()
                    .expect("compute built the topological order")
                    .order
                    .iter()
                    .rev()
                    .map(|&i| TaskId(i))
                    .find(|candidate| moved.contains(candidate))
                    .expect("the moved tasks are in the topological order");

                assert_ne!(want, before, "seed {seed}, {task}");
                let mut sol = before.clone();
                assert_eq!(
                    with_inject(
                        Inject {
                            fail: Some(fail),
                            ..Inject::default()
                        },
                        || edited.recompute_cone(&mut sol, task),
                    ),
                    Err(ScheduleError::Infeasible(fail)),
                    "seed {seed}, {task}, failing {fail}"
                );
                assert_eq!(sol, before, "seed {seed}, {task}, failing {fail}");

                edited.recompute_cone(&mut sol, task).unwrap();
                assert_eq!(sol, want, "seed {seed}, {task}, after failing {fail}");
                exercised += 1;
            }
        }
        assert!(exercised > 20, "only {exercised} rollback cases tried");
    }
}
