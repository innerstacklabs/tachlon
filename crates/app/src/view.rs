//! Viewport mutators: the canvas size, wheel, scroll, fit and hover, which the shell and the
//! tests drive through `Session` now that the timeline is private (gs#159).

use crate::Session;
use engine::{Schedule, TaskId};

impl Session {
    /// The canvas is now `width` x `height` CSS pixels at `dpr`.
    pub fn resize(&mut self, width: f64, height: f64, dpr: f64) {
        #[expect(clippy::float_cmp, reason = "any change of the stored width counts")]
        let widened = width != self.timeline.viewport.width;
        self.timeline.resize(width, height, dpr);
        if self.fit_pending && widened {
            crate::fit::fit_first_screen(&mut self.timeline, &self.solution);
        }
        self.link_refresh();
    }

    /// A wheel event at (`x`, `y`) with deltas in pixels; `zoom` (ctrl or pinch) zooms anchored at
    /// `x`, otherwise the wheel scrolls rows and pans time. Ignored during a bar drag.
    pub fn wheel(&mut self, x: f64, y: f64, dx: f64, dy: f64, zoom: bool) {
        self.fit_pending = false;
        self.snap_slide();
        self.reports.motion.fly = None;
        self.timeline.wheel(x, y, dx, dy, zoom);
        self.link_refresh();
    }

    /// Scroll the rows by `dy` pixels (positive is down the schedule), clamped to the schedule.
    pub fn scroll_by(&mut self, dy: f64) {
        // The table's own scrolling arrives here too, so a scroll ends the opening fit as a canvas wheel does; a zero move (the table
        // syncing its position after an open) does not.
        self.fit_pending &= dy == 0.0;
        if dy != 0.0 {
            self.snap_slide();
        }
        self.timeline.viewport.scroll_by(dy);
        self.link_refresh();
    }

    /// The table's sideways scrollbar is `px` tall: the rows scroll that much further, so the last row clears it.
    pub fn set_scroll_slack(&mut self, px: f64) {
        self.timeline.set_scroll_slack(px);
    }

    /// Highlight `row` as hovered while the pointer is outside the canvas (the task table's hovered
    /// row); `None` clears it. The canvas pointer wins while it is over the canvas.
    pub fn set_hover_row(&mut self, row: Option<usize>) {
        self.timeline.set_outside_hover(row);
    }

    /// The pointer left the canvas. A bar drag keeps going under pointer capture, so only a pan
    /// drag ends here.
    pub fn pointer_leave(&mut self) {
        self.timeline.pointer_leave();
    }

    /// Show the instants `start` to `end` (minutes from the project origin) with a small margin on
    /// both sides.
    pub fn fit(&mut self, start: f64, end: f64) {
        self.fit_pending = false;
        self.snap_slide();
        self.timeline.viewport.fit(start, end);
        self.link_refresh();
    }

    /// Set the bar look: corner `radius` in CSS pixels (0 or more) and bar height as a share of
    /// the row (above 0, at most 1). Anything else, NaN included, is refused and changes nothing.
    ///
    /// # Errors
    /// A message naming the value when the radius or the height is out of range.
    pub fn set_bar_look(&mut self, radius: f64, height: f64) -> Result<(), String> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(format!("bar radius {radius} is not 0 or more"));
        }
        if !(height > 0.0 && height <= 1.0) {
            return Err(format!("bar height {height} is not in (0, 1]"));
        }
        let viewport = &mut self.timeline.viewport;
        (viewport.bar_radius, viewport.bar_ratio) = (radius, height);
        // The bar height sets a milestone's width, so the task under a still pointer can change.
        self.link_refresh();
        Ok(())
    }

    /// Set the shapes that tell the data date from the critical path without colour (tl#1215), in
    /// CSS pixels: the data-date line's `dash` (1 or more) and `gap` (0 or more), and the critical
    /// bar's `edge` (0 or more). Anything else, NaN included, is refused and changes nothing.
    ///
    /// # Errors
    /// A message naming the values when one is out of range.
    pub fn set_shape_look(&mut self, dash: f64, gap: f64, edge: f64) -> Result<(), String> {
        if !(dash >= 1.0 && gap >= 0.0 && edge >= 0.0 && (dash + gap + edge).is_finite()) {
            return Err(format!(
                "shape look {dash}, {gap}, {edge} is not a dash of 1 or more and a gap and edge of 0 or more"
            ));
        }
        let viewport = &mut self.timeline.viewport;
        (viewport.today_dash, viewport.crit_edge) = ((dash, gap), edge);
        Ok(())
    }

    /// Scroll so `row` is the top row, as far as the scroll range allows.
    #[expect(
        clippy::float_cmp,
        reason = "equal exactly when the scroll did not move"
    )]
    pub fn scroll_to_row(&mut self, row: usize) {
        let scrolled = self.timeline.viewport.scroll_y;
        self.timeline.viewport.scroll_to_row(row);
        // A move of the rows ends the opening fit, as `scroll_by` does; no move leaves it pending.
        self.fit_pending &= self.timeline.viewport.scroll_y == scrolled;
        if self.timeline.viewport.scroll_y != scrolled {
            self.snap_slide();
        }
        self.link_refresh();
    }
}

/// The calendar most tasks use (the lowest id on a tie), which the scene shades:
/// for an imported file that is the project calendar. With no live task, the first calendar
/// (a blank schedule's weekdays); `None` only when there is no calendar.
#[expect(
    clippy::cast_possible_truncation,
    reason = "index is a task slot and best a calendar index; Schedule::add_task and add_calendar cast their counts to u32 with no check against u32::MAX, so past 2^32 they wrap, as those ids do"
)]
pub(crate) fn shading_calendar(schedule: &Schedule) -> Option<engine::CalendarId> {
    let mut uses = vec![0usize; schedule.calendars().len()];
    for (index, task) in schedule.tasks().iter().enumerate() {
        if schedule.is_deleted(TaskId(index as u32)) {
            continue;
        }
        uses[task.calendar.0 as usize] += 1;
    }
    let (best, count) =
        uses.iter().enumerate().fold(
            (0, 0),
            |best, (id, &n)| if n > best.1 { (id, n) } else { best },
        );
    (count > 0 || !uses.is_empty()).then_some(engine::CalendarId(best as u32))
}
