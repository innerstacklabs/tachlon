//! The lookahead window (gs#96, journey J2 step 4): a start and a length in weeks that keeps
//! only the activities active in it.
//!
//! The window is the half-open span `[start, start + weeks * WEEK)` of instants (minutes from the
//! origin). An activity occupies `[es, ef)` of its early dates, so it intersects the window when
//! it starts before the window's end and finishes after the window's start. Both edges follow
//! from the half-open rule and are tested:
//!
//! - An activity that finishes exactly at the window's start (`ef == start`) is outside it: its
//!   last working minute is the one before.
//! - An activity that starts on the window's last day is inside it. One that starts exactly at
//!   the window's end (`es == end`) is outside it.
//! - A milestone (`es == ef`) occupies no span, so it is inside when its instant is in the
//!   window: `start <= es < end`.

use engine::{Instant, WEEK};

/// Weeks in a window that is not asked for another length (journey J2: "the next 3 weeks").
pub const DEFAULT_WEEKS: u32 = 3;
/// Most weeks a window may span (ten years); a longer one is a typo, not a lookahead.
pub const MAX_WEEKS: u32 = 520;

/// A lookahead window: from `start`, for `weeks` weeks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookahead {
    /// First instant of the window.
    pub start: Instant,
    /// Length in weeks, `1..=MAX_WEEKS`.
    pub weeks: u32,
}

impl Lookahead {
    /// A window, or `None` when `weeks` is 0 or above [`MAX_WEEKS`].
    #[must_use]
    pub fn new(start: Instant, weeks: u32) -> Option<Lookahead> {
        (1..=MAX_WEEKS)
            .contains(&weeks)
            .then_some(Lookahead { start, weeks })
    }

    /// The first instant after the window.
    #[must_use]
    pub fn end(self) -> Instant {
        self.start.saturating_add(i64::from(self.weeks) * WEEK)
    }

    /// Whether an activity with early start `es` and early finish `ef` is active in the window.
    #[must_use]
    pub fn intersects(self, es: Instant, ef: Instant) -> bool {
        active_in(self.start, self.end(), es, ef)
    }
}

/// [`Lookahead::intersects`]'s rule for any half-open span `[start, end)`, not only whole weeks.
#[must_use]
pub fn active_in(start: Instant, end: Instant, es: Instant, ef: Instant) -> bool {
    if es >= ef {
        start <= es && es < end
    } else {
        es < end && ef > start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::DAY;

    #[test]
    fn the_length_is_one_week_to_ten_years() {
        assert!(Lookahead::new(0, 0).is_none());
        assert!(Lookahead::new(0, 1).is_some());
        assert!(Lookahead::new(0, MAX_WEEKS).is_some());
        assert!(Lookahead::new(0, MAX_WEEKS + 1).is_none());
        assert_eq!(Lookahead::new(5, 3).unwrap().end(), 5 + 3 * WEEK);
    }

    #[test]
    fn the_window_is_half_open_on_both_edges() {
        let w = Lookahead::new(WEEK, 1).unwrap();
        let end = 2 * WEEK;
        // Ends exactly at the start: outside. One minute later: inside.
        assert!(!w.intersects(WEEK - DAY, WEEK));
        assert!(w.intersects(WEEK - DAY, WEEK + 1));
        // Starts on the last day: inside. Starts exactly at the end: outside.
        assert!(w.intersects(end - DAY + 480, end + DAY));
        assert!(w.intersects(end - 1, end + DAY));
        assert!(!w.intersects(end, end + DAY));
        // An activity that spans the whole window.
        assert!(w.intersects(0, 3 * WEEK));
        // A milestone is in when its instant is in the window.
        assert!(w.intersects(WEEK, WEEK));
        assert!(w.intersects(end - 1, end - 1));
        assert!(!w.intersects(WEEK - 1, WEEK - 1));
        assert!(!w.intersects(end, end));
    }
}
