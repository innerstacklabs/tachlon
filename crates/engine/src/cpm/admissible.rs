//! The admissible start instants of one task, and the late-start search over them.

use crate::calendar::{Calendar, DAY, Instant, Minutes, WEEK};
use crate::model::{DepId, ScheduleError, TaskId};

/// The earliest instant [`Admissible`] reads: `contains`, `next` and `prev` take an earlier one
/// as this, so each answers for every instant down to `i64::MIN` and `count` stays in range.
const FLOOR: Instant = Instant::MIN / 2;
/// The earliest date a late limit or a pin caps a late date at: an earlier one (a sentinel, no
/// date) reads as this. Half of [`FLOOR`], so [`Admissible::step_back`] has room below it.
pub(super) const LATE_FLOOR: Instant = FLOOR / 2;

/// The admissible starts of one task.
///
/// A task with a duration starts on a working instant of its calendar. A
/// zero-duration milestone may also sit on a working-period end (a milestone
/// after a task done at 17:00 Friday is at 17:00 Friday), and its finish
/// equals its start.
#[derive(Clone, Copy)]
pub struct Admissible<'a> {
    cal: &'a Calendar,
    duration: Minutes,
}

impl<'a> Admissible<'a> {
    /// The admissible starts of a task with this calendar and duration.
    #[must_use]
    pub fn new(cal: &'a Calendar, duration: Minutes) -> Admissible<'a> {
        Admissible { cal, duration }
    }

    fn milestone(&self) -> bool {
        self.duration == 0
    }

    /// Is minute `[t, t + 1)` working? Before the origin the weekly pattern runs on.
    fn working(&self, t: Instant) -> bool {
        if t >= 0 {
            self.cal.is_working(t)
        } else {
            self.cal.count(t + 1) > self.cal.count(t)
        }
    }

    /// Is `t` an admissible start? The origin is no period end, as before gantt#2072: an early
    /// milestone starts there only when its minute is working.
    #[must_use]
    pub fn contains(&self, t: Instant) -> bool {
        let t = t.max(FLOOR);
        self.working(t) || (self.milestone() && t != 0 && self.working(t - 1))
    }

    /// Smallest admissible start `>= t`.
    #[must_use]
    pub fn next(&self, t: Instant) -> Instant {
        let t = t.max(FLOOR);
        if self.contains(t) {
            t
        } else {
            self.cal.nth(self.cal.count(t))
        }
    }

    /// Largest admissible start `<= t` (`<= FLOOR` for a `t` before it). One always exists: the
    /// weekly pattern runs on before the origin.
    #[must_use]
    pub fn prev(&self, t: Instant) -> Instant {
        let t = t.max(FLOOR);
        if self.working(t) {
            return t;
        }
        // The last working minute before `t`; a milestone may sit at its end (not at the origin).
        let last = self.cal.nth(self.cal.count(t) - 1);
        if self.milestone() && last != -1 {
            last + 1
        } else {
            last
        }
    }

    /// Where the late-start search begins when a late limit binds: the first admissible instant
    /// if it meets `pred`, else a start that does, found stepping back twice as far each time.
    /// No step goes below the last admissible start a week after `FLOOR` (every week has one),
    /// so the loop ends; `None` when even that start misses `pred` (a duration or a lag of about
    /// 2^61 minutes under a limit at `LATE_FLOOR`).
    pub(super) fn step_back(&self, pred: impl Fn(Instant) -> bool) -> Option<Instant> {
        let (mut lo, mut step) = (self.next(0), DAY);
        while !pred(lo) {
            let prior = self.prev(lo.saturating_sub(step).max(FLOOR + WEEK));
            if prior >= lo {
                return None;
            }
            (lo, step) = (prior, step.saturating_mul(2));
        }
        Some(lo)
    }

    /// `fin(T, s) = add(cal T, s, dur T)`.
    #[must_use]
    pub fn fin(&self, s: Instant) -> Instant {
        self.cal.add(s, self.duration)
    }

    /// `earliest_start(T, pred)`: the earliest admissible start `s >= lo`
    /// with `pred(s)`, searching `t` in `[lo, probe_hi]` and testing
    /// `pred(next(t))`. `pred` must be monotone (false, then true). `None`
    /// when `pred(next(probe_hi))` is false.
    ///
    /// `probe_hi` bounds the probes, not the result: the result is
    /// `next(t)` for the smallest such `t`, and is past `probe_hi` when no
    /// admissible start lies in `[t, probe_hi]` (a `probe_hi` on a weekend
    /// can give Monday morning). The forward pass relies on this: its
    /// `probe_hi` is `max(lo, finish bound)`, which need not be an admissible
    /// start.
    pub fn earliest(
        &self,
        lo: Instant,
        probe_hi: Instant,
        pred: impl Fn(Instant) -> bool,
    ) -> Option<Instant> {
        let (mut a, mut b) = (lo, probe_hi.max(lo));
        if !pred(self.next(b)) {
            return None;
        }
        while a < b {
            let mid = a + (b - a) / 2;
            if pred(self.next(mid)) {
                b = mid;
            } else {
                a = mid + 1;
            }
        }
        Some(self.next(a))
    }

    /// `latest_start(T, pred)`: the latest admissible start in `[lo, hi]`
    /// with `pred`, searching `t` and testing `pred(prev(t))`. `pred` must be
    /// monotone (true, then false). `None` when no admissible start in
    /// `[lo, hi]` satisfies it.
    pub fn latest(
        &self,
        lo: Instant,
        hi: Instant,
        pred: impl Fn(Instant) -> bool,
    ) -> Option<Instant> {
        let first = self.next(lo);
        if first > hi || !pred(first) {
            return None;
        }
        let (mut a, mut b) = (first, hi);
        while a < b {
            let mid = a + (b - a + 1) / 2;
            if pred(self.prev(mid)) {
                a = mid;
            } else {
                b = mid - 1;
            }
        }
        Some(self.prev(a))
    }
}

/// The late start and finish of `task`: the latest admissible start in `[e, hi]`
/// with `pred` (monotone: `Some`, then `None`), where `e` is its early start.
///
/// The early start must satisfy `pred` (es <= ls). It is checked, not
/// assumed: when it fails, the engine has a bug, reported as
/// [`ScheduleError::InvariantViolated`] with the dependency `blame` names and
/// the latest start at or after the origin that does satisfy `pred`, if any does. A guess that
/// satisfies `pred` settles the check without probing `e`: `pred` is monotone
/// and `e <= g`.
///
/// `guess` narrows the search and never changes its answer. The largest
/// admissible start `g <= guess` with `g >= e` splits the domain: if `pred(g)`
/// returns its finish, the answer is in `[g, hi]`, and it is `g` itself when
/// the next admissible start after `g` is past `hi` or fails `pred`;
/// otherwise the answer is in `[e, g - 1]` (`g > e` then, since `pred(e)`
/// holds). On that guess path, the returned finish is the one `pred`
/// computed.
pub(super) fn late_start(
    a: &Admissible,
    task: TaskId,
    e: Instant,
    hi: Instant,
    guess: Instant,
    pred: impl Fn(Instant) -> Option<Instant>,
    blame: impl FnOnce() -> Option<DepId>,
) -> Result<(Instant, Instant), ScheduleError> {
    let g = Some(a.prev(guess.min(hi))).filter(|&g| g >= e);
    let fin_g = g.and_then(&pred);
    // `pred` is monotone and `e <= g`: a `pred(g)` that holds makes `pred(e)` hold, so it is
    // probed only when the guess did not settle it.
    if fin_g.is_none() && pred(e).is_none() {
        return Err(ScheduleError::InvariantViolated {
            task,
            dep: blame(),
            es: e,
            ls: a.latest(0, hi, |x| pred(x).is_some()),
        });
    }
    let (mut lo, mut hi) = (e, hi);
    if let Some(g) = g {
        if let Some(fin_g) = fin_g {
            let after = a.next(g + 1);
            if after > hi || pred(after).is_none() {
                return Ok((g, fin_g));
            }
            lo = g;
        } else {
            hi = g - 1;
        }
    }
    let x = a
        .latest(lo, hi, |x| pred(x).is_some())
        .expect("lo is admissible and satisfies every predicate");
    Ok((x, a.fin(x)))
}

#[cfg(test)]
mod tests;
