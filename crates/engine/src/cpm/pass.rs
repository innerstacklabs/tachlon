//! The per-task steps of the forward and backward passes, shared by the full solve and the cone.

mod lag;

use super::TaskDates;
use super::admissible::{Admissible, LATE_FLOOR, late_start};
use crate::calendar::{Calendar, Instant, Minutes};
use crate::model::options::UnstartedRemaining;
use crate::model::{DepId, Dependency, Graph, LinkType, Schedule, ScheduleError, Task, TaskId};
use lag::{Choice, LagSource};

/// The borrowed parts of a [`Schedule`] one pass reads, and the per-task
/// steps of the forward and backward passes. [`Schedule::compute`] runs them
/// over every task; [`Schedule::recompute_cone_moved`] and
/// [`Schedule::recompute_duration_cone_moved`] run them over the tasks a
/// change can reach. One implementation of each step, so the two
/// cannot disagree about a date.
pub(super) struct Pass<'a> {
    pub(super) tasks: &'a [Task],
    pub(super) deps: &'a [Dependency],
    pub(super) cals: &'a [Calendar],
    pub(super) g: &'a Graph,
    pub(super) lag: LagSource<'a>,
    /// The data date ([`Schedule::set_data_date`]), the origin when none is set.
    pub(super) data_date: Instant,
    /// Some task carries a [`Limit`](crate::model::Limit) ([`Schedule::set_limit`]).
    pub(super) limited: bool,
    /// Whether an unstarted task's remaining duration sets its length ([`Task::work`]).
    pub(super) unstarted_remaining: UnstartedRemaining,
}

impl Pass<'_> {
    /// Total float of task `i` (decision 0008): a finished task has none, a started task's is
    /// the working minutes of its calendar from its early finish to its late finish, an
    /// unstarted task's from its early start to its late start.
    #[inline]
    pub(super) fn float_of(&self, i: usize, d: &TaskDates) -> Minutes {
        let t = &self.tasks[i];
        if t.actual_finish.is_some() {
            0
        } else if t.actual_start.is_some() {
            self.cal_of(i).working_between(d.ef, d.lf)
        } else {
            self.cal_of(i).working_between(d.es, d.ls)
        }
    }

    /// The start and finish dates task `i` is pinned to, snapped to working time once for both
    /// passes: a start to the next admissible start, a finish to the end of the last working
    /// minute at or before it (the date itself when none precedes it). A date off working time,
    /// such as midnight or a weekend, then meets the pin exactly and the pinned task keeps 0.
    fn pin_dates(&self, i: usize) -> (Option<Instant>, Option<Instant>) {
        let (limit, cal) = (self.tasks[i].limit, self.cal_of(i));
        let start = limit.start_pin(self.tasks[i].actual_start.is_some());
        let finish = limit.finish_pin();
        (
            start.map(|c| self.adm(i).next(c.max(0))),
            finish.map(|c| cal.prev_period_end(c).unwrap_or(c)),
        )
    }

    /// The start and finish no-later-than bounds on task `i`'s late dates, pins included, floored
    /// at `LATE_FLOOR`.
    fn late_limits(&self, i: usize) -> (Option<Instant>, Option<Instant>) {
        let task = &self.tasks[i];
        let (start_pin, finish_pin) = self.pin_dates(i);
        let by_start = task.limit.snlt.filter(|_| task.actual_start.is_none());
        let floored = |c: Option<Instant>| c.map(|c| c.max(LATE_FLOOR));
        (
            floored(start_pin.into_iter().chain(by_start).min()),
            floored(finish_pin.into_iter().chain(task.limit.fnlt).min()),
        )
    }

    pub(super) fn cal_of(&self, i: usize) -> &Calendar {
        &self.cals[self.tasks[i].calendar.0 as usize]
    }

    /// The calendar the lag of `d` is counted in (the schedule's lag
    /// calendar). The admissible-start search still uses the successor's own
    /// calendar: a lag lands on an instant, and the successor then starts at
    /// its next admissible instant.
    #[inline]
    fn lag_cal(&self, di: u32, d: &Dependency) -> &Calendar {
        self.lag.cal(self.cals, self.tasks, di as usize, d)
    }

    /// The admissible starts of the work the passes schedule for task `i`: its remaining work
    /// once started ([`Task::work`]), else its whole duration.
    fn adm(&self, i: usize) -> Admissible<'_> {
        let task = &self.tasks[i];
        Admissible::new(self.cal_of(i), task.work(self.unstarted_remaining))
            .kind(task.limit.milestone)
    }

    pub(super) fn preds(&self, i: usize) -> &[u32] {
        &self.g.pred_deps[self.g.pred_start[i] as usize..self.g.pred_start[i + 1] as usize]
    }

    pub(super) fn succs(&self, i: usize) -> &[u32] {
        &self.g.succ_deps[self.g.succ_start[i] as usize..self.g.succ_start[i + 1] as usize]
    }

    /// The early start and finish of task `si`, given the early dates of its
    /// predecessors (`early(p) = (es, ef)`).
    ///
    /// Per dependency of S on P, a predicate on S's start s:
    ///   FS: s >= add(cal L, ef P, lag)       SS: s >= add(cal L, es P, lag)
    ///   FF: fin(S, s) >= add(cal L, ef P, lag)
    ///   SF: fin(S, s) >= `finish_floor`(cal S, add(cal L, es P, lag))
    ///   SNET: s >= c;   data date: s >= dd;   always s >= origin.
    ///   FNET: fin(S, s) >= c, one more finish bound.
    ///   Pin: s >= c (start) or fin(S, s) >= c (finish), and no dependency binds.
    /// A started task keeps its actual start as its early start, and the predicates bound its
    /// remaining work instead (SNET does not apply); a finished task has its actual dates and
    /// no predicate.
    /// where `cal L` is the lag calendar of the dependency ([`Pass::lag_cal`]).
    /// The `s >= x` predicates are one lower bound `lo`; the `fin >= x`
    /// predicates are one bound `fin_lo`; es is the earliest admissible
    /// start satisfying both.
    #[inline]
    pub(super) fn forward(
        &self,
        si: u32,
        early: impl Fn(usize) -> (Instant, Instant),
    ) -> Result<(Instant, Instant), ScheduleError> {
        let s = si as usize;
        let t = &self.tasks[s];
        // A finished task is fixed at its actuals: no predecessor, constraint or data date moves
        // it.
        if let Some(done) = t.actual_finish {
            let start = t.actual_start.unwrap_or(done).max(0);
            return Ok((start, done.max(start)));
        }
        // A started task keeps its actual start. Its remaining work cannot begin before the data
        // date or before the task did, and waits for its predecessors (retained logic, decision
        // 0008); its own SNET no longer binds. An unstarted task starts no earlier than the data
        // date.
        let pin = t.actual_start.map(|a| a.max(0));
        let mut lo = match pin {
            None => t.snet.unwrap_or(0).max(self.data_date).max(0),
            // The remaining work also waits for its resume date.
            Some(a) => a.max(self.data_date).max(t.limit.resume.unwrap_or(0)),
        };
        // A pin holds its date whatever the predecessors do: none binds it, and each shows as
        // negative float when it runs past the date.
        let (start_pin, finish_pin) = self.pin_dates(s);
        lo = lo.max(start_pin.unwrap_or(0));
        let mut fin_lo = t.limit.fnet.max(finish_pin);
        let held = start_pin.or(finish_pin).is_some();
        for &di in self.preds(s).iter().filter(|_| !held) {
            let d = &self.deps[di as usize];
            let (pred_start, pred_finish) = early(d.pred.0 as usize);
            let cal_l = self.lag_cal(di, d);
            match d.kind {
                LinkType::FS => lo = lo.max(cal_l.add(pred_finish, d.lag)),
                LinkType::SS => lo = lo.max(cal_l.add(pred_start, d.lag)),
                LinkType::FF => fin_lo = fin_lo.max(Some(cal_l.add(pred_finish, d.lag))),
                LinkType::SF => {
                    let x = finish_floor(self.cal_of(s), cal_l.add(pred_start, d.lag));
                    fin_lo = fin_lo.max(Some(x));
                }
            }
        }
        let a = self.adm(s);
        let start = match fin_lo {
            None => a.next(lo),
            // fin(S, next(x)) >= next(x) >= x, so probe_hi = max(lo, x)
            // satisfies it; the start found may be past x (Admissible::earliest).
            Some(x) => a
                .earliest(lo, lo.max(x), |t| a.fin(t) >= x)
                .ok_or(ScheduleError::Infeasible(TaskId(si)))?,
        };
        Ok((pin.unwrap_or(start), a.fin(start)))
    }

    /// The late start and finish of task `pi`, given the project finish
    /// `pf`, its own early start `e`, and the late dates of its successors
    /// (`late(s) = (ls, lf)`).
    ///
    /// Per dependency of S on P, a predicate on P's start x (ls S and lf S
    /// already known):
    ///   FS: add(cal L, fin(P, x), lag) <= ls S    SS: add(cal L, x, lag) <= ls S
    ///   FF: add(cal L, fin(P, x), lag) <= lf S    SF: add(cal L, x, lag) <= `next_working`(cal S, lf S)
    ///   always: fin(P, x) <= PF.   SNET does not bind here.
    ///   SNLT: x <= c (an unstarted task);   FNLT: fin(P, x) <= c;   a pin is one of the two.
    ///
    /// The result depends on `pf`, the successors' late dates and the task
    /// itself, never on the early start: `e` only checks that the early
    /// start is feasible (es <= ls) and bounds the search from below.
    ///
    /// Progress (decision 0008): a started task's late start and finish are those of its
    /// remaining work; a finished task's are its actuals, and a finished successor binds no
    /// predecessor. A started task's SS and SF successors bind its fixed actual start, as in the
    /// forward pass, not its remaining work. A started task ignores its start-no-later-than.
    ///
    /// A late limit no start can meet puts the late start before the early start: the float is
    /// negative. With no limit anywhere that is an engine bug and an error; with one, the late
    /// search starts at the first admissible instant and, when even that misses a limit, steps
    /// back before the origin, where the weekly pattern runs on (gantt#2072).
    #[inline]
    pub(super) fn backward(
        &self,
        pi: u32,
        pf: Instant,
        e: Instant,
        late: impl Fn(usize) -> (Instant, Instant),
    ) -> Result<(Instant, Instant), ScheduleError> {
        self.backward_with(pi, pf, e, late, true)
    }

    /// [`Pass::backward`], with the task's own late limits and pins left out when `own_limits`
    /// is false.
    #[inline]
    pub(super) fn backward_with(
        &self,
        pi: u32,
        pf: Instant,
        e: Instant,
        late: impl Fn(usize) -> (Instant, Instant),
        own_limits: bool,
    ) -> Result<(Instant, Instant), ScheduleError> {
        let p = pi as usize;
        let task = &self.tasks[p];
        // A finished task cannot move: its late dates are its actual dates.
        if let Some(done) = task.actual_finish {
            return Ok((e, done.max(e)));
        }
        let adm = self.adm(p);
        let succs = self.succs(p);
        // A started task's start is its fixed actual start, which the forward pass hands its
        // successors as the early start: an SS or SF successor bounds that fixed start (already
        // met, since the successor's early start is no earlier), never the remaining work.
        let started = task.actual_start.is_some();
        let binds_work = |kind: LinkType| !(started && matches!(kind, LinkType::SS | LinkType::SF));
        let holds = |x: Instant, fin_x: Instant, di: u32| -> bool {
            let d = &self.deps[di as usize];
            let s = d.succ.0 as usize;
            // A finished successor is fixed: it does not bind its predecessors.
            if self.tasks[s].actual_finish.is_some() || !binds_work(d.kind) {
                return true;
            }
            let cal_l = self.lag_cal(di, d);
            let (succ_late_start, succ_late_finish) = late(s);
            match d.kind {
                LinkType::FS => cal_l.add(fin_x, d.lag) <= succ_late_start,
                LinkType::SS => cal_l.add(x, d.lag) <= succ_late_start,
                LinkType::FF => cal_l.add(fin_x, d.lag) <= succ_late_finish,
                LinkType::SF => {
                    cal_l.add(x, d.lag) <= next_working(self.cal_of(s), succ_late_finish)
                }
            }
        };
        let (snlt, fnlt) = if own_limits {
            self.late_limits(p)
        } else {
            (None, None)
        };
        let pred = |x: Instant| {
            let f = adm.fin(x);
            (f <= pf
                && snlt.is_none_or(|c| x <= c)
                && fnlt.is_none_or(|c| f <= c)
                && succs.iter().all(|&d| holds(x, f, d)))
            .then_some(f)
        };
        // Upper bound: x <= fin(P, x) <= PF; with a lag >= 0,
        // add(cal L, y, lag) >= y (for every lag calendar), and both y = x
        // and y = fin(P, x) are >= x.
        //
        // Guess: each inequality read backwards (a lag subtracted, the
        // duration subtracted) gives a candidate for the latest start.
        // `add` has no inverse, so the guess can be wrong; it only narrows
        // the search, and the answer is still the latest start the
        // forward predicates accept (see `late_start`).
        let cal_p = self.cal_of(p);
        let fin_top = fnlt.map_or(pf, |c| c.min(pf));
        let start_top = snlt.unwrap_or(Instant::MAX);
        let (mut hi, mut fin_cap, mut start_cap) = (fin_top.min(start_top), fin_top, start_top);
        for &di in succs {
            let d = &self.deps[di as usize];
            let s = d.succ.0 as usize;
            if self.tasks[s].actual_finish.is_some() || !binds_work(d.kind) {
                continue;
            }
            let (succ_late_start, succ_late_finish) = late(s);
            let bound = match d.kind {
                LinkType::FS | LinkType::SS => succ_late_start,
                LinkType::FF => succ_late_finish,
                // A start at the next working instant meets a finish at a period end.
                LinkType::SF => next_working(self.cal_of(s), succ_late_finish),
            };
            if d.lag >= 0 {
                hi = hi.min(bound);
            }
            let back = self.lag_cal(di, d).add(bound, -d.lag);
            match d.kind {
                LinkType::FS | LinkType::FF => fin_cap = fin_cap.min(back),
                LinkType::SS | LinkType::SF => start_cap = start_cap.min(back),
            }
        }
        let guess = start_cap.min(cal_p.add(fin_cap, -task.work(self.unstarted_remaining)));
        let blame = || {
            let fe = adm.fin(e);
            succs.iter().copied().find(|&d| !holds(e, fe, d)).map(DepId)
        };
        if !self.limited {
            return late_start(&adm, TaskId(pi), e, hi, guess, pred, blame);
        }
        // The first admissible instant can itself miss a bound: step back from it, twice as far
        // each time, to a start that meets every bound, and search from there.
        let lo = adm
            .step_back(|x| pred(x).is_some())
            .ok_or(ScheduleError::Infeasible(TaskId(pi)))?;
        late_start(&adm, TaskId(pi), lo, hi, guess, pred, blame)
    }

    /// Place each unstarted as-late-as-possible task, last in the order first, at the latest start
    /// that moves no successor: the backward step with the successors' early dates as their late
    /// dates, so a chain of such tasks packs against the first one that does not move. The start
    /// only ever moves later. `pf` stands: no finish passes it.
    pub(super) fn alap(
        &self,
        pf: Instant,
        es: &mut [Instant],
        ef: &mut [Instant],
    ) -> Result<(), ScheduleError> {
        for &pi in self.g.order.iter().rev() {
            let p = pi as usize;
            let t = &self.tasks[p];
            if !t.limit.alap || t.deleted || t.actual_start.is_some() {
                continue;
            }
            let (start, finish) = self.backward(pi, pf, es[p], |s| (es[s], ef[s]))?;
            if start > es[p] {
                (es[p], ef[p]) = (start, finish);
            }
        }
        Ok(())
    }

    /// The backward pass over every task, from the project finish `pf`.
    pub(super) fn backward_all(
        &self,
        pf: Instant,
        es: impl Fn(usize) -> Instant,
    ) -> Result<(Vec<Instant>, Vec<Instant>), ScheduleError> {
        let n = self.tasks.len();
        let mut ls = vec![0 as Instant; n];
        let mut lf = vec![0 as Instant; n];
        for &pi in self.g.order.iter().rev() {
            let p = pi as usize;
            (ls[p], lf[p]) = self.backward(pi, pf, es(p), |s| (ls[s], lf[s]))?;
        }
        Ok((ls, lf))
    }
}

/// The smallest working instant of `cal` at or after `t`, before the origin too: a late finish
/// can fall there, where [`Calendar::next_working`] answers from the origin on.
fn next_working(cal: &Calendar, t: Instant) -> Instant {
    cal.nth(cal.count(t))
}

/// The earliest finish, in the successor's calendar `cal`, that meets an SF bound `x`: a finish at
/// a working-period end meets a bound at the start of the next period, with no working minute
/// between them (16:00 Monday meets 08:00 Tuesday), as `next_working` reads it backward.
pub(super) fn finish_floor(cal: &Calendar, x: Instant) -> Instant {
    match cal.prev_period_end(x) {
        Some(e) if cal.working_between(e, x) == 0 => e,
        _ => x,
    }
}

impl Schedule {
    /// The resolved lag calendars (the schedule's and each dependency's own), for the passes and
    /// `dependency_start`.
    pub(super) fn lag_source(&self) -> LagSource<'_> {
        let all_day = self.all_day.as_ref();
        LagSource {
            default: Choice::resolve(self.lag_calendar, &self.calendars, all_day),
            own: &self.lag_over,
            all_day,
        }
    }
}
