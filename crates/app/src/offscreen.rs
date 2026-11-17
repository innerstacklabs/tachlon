//! Where the work went (gs#309, idea c): when no row in the window has a bar in the window's
//! time span, the nearest finish before it and the nearest start after it, for the rows that are
//! in the window. The page shows them as one note at the edge (`packages/gantt/src/offscreen.ts`).
//!
//! The scan reads the rows the window shows, a few dozen, never the task count, and runs when
//! `status_json` does. It is not kept between frames: `Session` is at its size budget, and the
//! scan is one read of the visible rows.

use crate::Session;
use engine::Instant;

/// The work around an empty window: the latest finish before it and the earliest start after it,
/// among the rows the window shows. Either is `None` when no such row has work there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offscreen {
    pub before: Option<Instant>,
    pub after: Option<Instant>,
}

impl Session {
    /// A bar's extent on `task`'s row: a summary's span, else the early dates.
    fn row_extent(&self, task: u32) -> Option<(Instant, Instant)> {
        self.timeline().summaries().get(task).or_else(|| {
            self.solution()
                .dates
                .get(task as usize)
                .map(|d| (d.es, d.ef))
        })
    }

    /// The nearest finish before and start after the window, when no visible row has work in the
    /// window's time span; `None` when one has, when no row shows, or when no row has work
    /// outside the span either.
    pub fn offscreen(&self) -> Option<Offscreen> {
        let v = &self.timeline().viewport;
        let (from, to) = v.visible_time();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "from and to are a time window in minutes; a float-to-int `as` saturates, so an out-of-range value clamps and NaN becomes 0, and it never wraps"
        )]
        let (from, to) = (from.floor() as Instant, to.ceil() as Instant);
        let rows = v.visible_rows();
        let order = self.timeline().rows();
        let mut before: Option<Instant> = None;
        let mut after: Option<Instant> = None;
        let mut shown = false;
        for &task in order.get(rows)? {
            let Some((es, ef)) = self.row_extent(task) else {
                continue;
            };
            shown = true;
            if es <= to && ef >= from {
                return None;
            }
            if ef < from {
                // The last minute of the bar, so a finish at midnight reads as the day before.
                before = before.max(Some(if ef > es { ef - 1 } else { ef }));
            } else {
                after = Some(after.map_or(es, |a| a.min(es)));
            }
        }
        (shown && (before.is_some() || after.is_some())).then_some(Offscreen { before, after })
    }

    /// The `offscreen` member of the status: `null`, or `{"b":"12 Jan","a":"2 Aug"}` with `null`
    /// for a side that has no work. At most 60 bytes more than `null`.
    pub(crate) fn offscreen_json(&self) -> String {
        let Some(o) = self.offscreen() else {
            return "null".to_string();
        };
        let side =
            |t: Option<Instant>| t.map_or("null".to_string(), |t| crate::quote(&self.day_month(t)));
        format!(r#"{{"b":{},"a":{}}}"#, side(o.before), side(o.after))
    }

    /// Pans so the work the note names comes into view, the rows staying where they are: the next
    /// start (`later`) a fifth in from the left, or the last finish a fifth in from the right.
    /// False, and nothing changes, when the window is not empty of work, that side has none, or a
    /// bar press or drag is in progress. It selects nothing and takes no focus.
    pub fn go_to_offscreen(&mut self, later: bool) -> bool {
        if self.busy() {
            return false;
        }
        let Some(o) = self.offscreen() else {
            return false;
        };
        let width = self.timeline().viewport.width;
        let (at, want) = match (later, o.before, o.after) {
            (true, _, Some(t)) => (t, width * 0.2),
            (false, Some(t), _) => (t, width * 0.8),
            _ => return false,
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        let x = self.timeline().viewport.x_of(at as f64);
        self.fit_pending = false;
        self.timeline.viewport.pan(want - x, 0.0);
        true
    }
}
