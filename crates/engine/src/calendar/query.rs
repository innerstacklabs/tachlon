//! The queries on a [`Calendar`]: `count`, `nth` and what is built on them.
//!
//! `count`, `nth`, `add` and `working_between` run on before the origin, where the weekly pattern
//! continues with no exception day: a late date can fall there (gantt#2072). The neighbour
//! queries (`is_working`, `next_working`, `prev_working`, `prev_period_end`, `day_start_at`) stay
//! at or after the origin, where every early date is.

use super::index::{first_not_before, minute_at};
use super::{Calendar, DAY, Instant, Minutes, WEEK};

impl Calendar {
    /// `C(t)`: working minutes in `[0, t)`; for `t < 0`, minus the pattern working minutes in
    /// `[t, 0)`.
    #[must_use]
    pub fn count(&self, t: Instant) -> Minutes {
        if t < 0 {
            let weeks = t.div_euclid(WEEK);
            return weeks * self.weekly + self.pattern_count(t - weeks * WEEK);
        }
        let day = t / DAY;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "t >= 0 here (a negative t returned above); on a 32-bit target a day past 2^32 \
                      (about 11.7 million years) would read another day's entry (unchecked)"
        )]
        if let Some(&entry) = self.day_table.get(day as usize) {
            if entry & 1 == 0 {
                #[expect(clippy::cast_lossless, reason = "i32 widens to i64 exactly")]
                return self.pattern_count(t) - (entry >> 1) as i64;
            }
            #[expect(
                clippy::cast_sign_loss,
                reason = "an exception day's entry is index << 1 | 1, which is non-negative"
            )]
            return self.exception_count((entry >> 1) as usize, t);
        }
        if let Some(removed) = self.tail_removed {
            return self.pattern_count(t) - removed;
        }
        self.count_search(t)
    }

    /// Search-based `count`, used past a truncated dense table.
    pub(super) fn count_search(&self, t: Instant) -> Minutes {
        let day = t / DAY;
        let n = self.exception_index(day);
        if n < self.exdays.len() && self.exdays[n].day == day {
            return self.exception_count(n, t);
        }
        self.pattern_count(t) - self.removed[n]
    }

    /// Instant of the `k`-th working minute (0-based); for `k < 0`, of the pattern working minute
    /// `-k` back from the origin.
    #[must_use]
    pub fn nth(&self, k: Minutes) -> Instant {
        if k < 0 {
            let weeks = k.div_euclid(self.weekly);
            return weeks * WEEK + self.pattern_instant(k - weeks * self.weekly);
        }
        let n = first_not_before(&self.key, &self.by_key, k / self.weekly, |&f| f <= k);
        // The last exception day starting at or before minute `k`: when `k`
        // falls inside its minutes, the day's own periods place it.
        if n > 0 {
            let r = k - self.key[n - 1];
            if r < self.exdays[n - 1].minutes {
                let start = self.exdays[n - 1].day * DAY;
                return start + minute_at(self.exception_periods(n - 1), r);
            }
        }
        self.pattern_instant(k + self.removed[n])
    }

    /// Is minute `[t, t + 1)` working? An exception day is checked before the
    /// pattern, since its periods may work where the pattern does not.
    #[must_use]
    pub fn is_working(&self, t: Instant) -> bool {
        if t < 0 {
            return false;
        }
        let day = t / DAY;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "t >= 0 here (a negative t returned above); on a 32-bit target a day past \
                      2^32 (about 11.7 million years) would read another day's entry (unchecked)"
        )]
        if let Some(&entry) = self.day_table.get(day as usize) {
            if entry & 1 == 1 {
                #[expect(
                    clippy::cast_sign_loss,
                    reason = "an exception day's entry is index << 1 | 1, which is non-negative"
                )]
                return self.exception_working((entry >> 1) as usize, t);
            }
        } else if self.tail_removed.is_none() {
            let n = self.exception_index(day);
            if n < self.exdays.len() && self.exdays[n].day == day {
                return self.exception_working(n, t);
            }
        }
        #[expect(
            clippy::cast_sign_loss,
            reason = "t >= 0 here (a negative t returned above), so t % WEEK is in 0..WEEK"
        )]
        let o = (t % WEEK) as usize;
        self.cum_week[o + 1] != self.cum_week[o]
    }

    /// Working minutes between `a` and `b` (`C(b) - C(a)`; negative if `b < a`).
    #[must_use]
    pub fn working_between(&self, a: Instant, b: Instant) -> Minutes {
        self.count(b) - self.count(a)
    }

    /// Smallest working instant `>= t`. Always exists. An instant before the
    /// origin has no working minute at or before it, so the answer is the first
    /// working instant at or after the origin.
    #[must_use]
    pub fn next_working(&self, t: Instant) -> Instant {
        self.nth(self.count(t.max(0)))
    }

    /// Largest working instant `<= t`, if any. `None` for an instant before the
    /// origin: no working minute precedes it.
    #[must_use]
    pub fn prev_working(&self, t: Instant) -> Option<Instant> {
        if t < 0 {
            return None;
        }
        if self.is_working(t) {
            return Some(t);
        }
        let c = self.count(t);
        (c > 0).then(|| self.nth(c - 1))
    }

    /// The first working minute of the working day that `next_working(t)`
    /// falls on: where a start asked for at `t` opens that day. `t` before the
    /// origin gives the first working day's start.
    #[must_use]
    pub fn day_start_at(&self, t: Instant) -> Instant {
        let day = self.next_working(t) / DAY;
        self.next_working(day * DAY)
    }

    /// Is `t` the end of a working period (minute `t - 1` working)?
    #[must_use]
    pub fn is_period_end(&self, t: Instant) -> bool {
        t > 0 && self.is_working(t - 1)
    }

    /// Largest working-period end `<= t`, if any. `None` for an instant before
    /// the origin: no working period ends before it.
    #[must_use]
    pub fn prev_period_end(&self, t: Instant) -> Option<Instant> {
        if t < 0 {
            return None;
        }
        if self.is_period_end(t) {
            return Some(t);
        }
        let c = self.count(t);
        (c > 0).then(|| self.nth(c - 1) + 1)
    }

    /// `add(cal, t, w)`: from instant `t`, move `w` working minutes
    /// forward (`w > 0`) or backward (`w < 0`).
    ///
    /// * `w > 0`: the smallest `u` with `w` working minutes in `[t, u)`. A
    ///   result therefore lands on a period end, never on the next period's
    ///   start (17:00 Friday, not 08:00 Monday).
    /// * `w < 0`: the largest `u` with `-w` working minutes in `[u, t)`, before the origin when
    ///   fewer than `-w` working minutes lie between it and `t`.
    /// * `w == 0`: `t` unchanged.
    ///
    /// Monotone non-decreasing in `t`. It collapses non-working gaps, so it has
    /// no inverse; the scheduler never tries to invert it.
    #[must_use]
    pub fn add(&self, t: Instant, w: Minutes) -> Instant {
        if w == 0 {
            return t;
        }
        let c = self.count(t);
        if w > 0 {
            self.nth(c + w - 1) + 1
        } else {
            self.nth(c + w)
        }
    }
}
