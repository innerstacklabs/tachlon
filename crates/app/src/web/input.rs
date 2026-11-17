//! Pointer, wheel, hover, scroll, and coordinate methods.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Wheel at (x, y) with deltas in pixels; `zoom` for ctrl/pinch.
    pub fn wheel(&mut self, x: f64, y: f64, dx: f64, dy: f64, zoom: bool) {
        self.session.wheel(x, y, dx, dy, zoom);
    }

    /// Show the earliest start of any live task to the project finish (the Fit button).
    pub fn fit_project(&mut self) -> bool {
        self.session.fit_project()
    }

    /// Pointer pressed at (x, y). A press on the link terminal, or with `alt` held on a leaf bar,
    /// starts a link drag.
    pub fn pointer_down(&mut self, x: f64, y: f64, alt: bool) {
        self.session.pointer_down(x, y, alt);
    }

    /// Whether a bar drag is in progress (the shell sets the cursor).
    pub fn dragging_bar(&self) -> bool {
        self.session.drag_started()
    }

    /// Pointer moved to (x, y).
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        self.session.pointer_move(x, y);
    }

    /// Pointer released.
    pub fn pointer_up(&mut self) {
        self.session.pointer_up();
    }

    /// Clear the selected task, its driving path and the selected link.
    pub fn clear_selection(&mut self) {
        self.session.clear_selection();
    }

    /// The gesture was cancelled (`pointercancel`, or Escape): a bar drag is undone and a link drag
    /// adds nothing.
    pub fn pointer_cancel(&mut self) {
        self.session.pointer_cancel();
    }

    /// Jump to the next critical task (the `c` key). Its row, if any.
    pub fn next_critical(&mut self) -> Option<u32> {
        self.session.next_critical()
    }

    /// Pointer left the canvas.
    pub fn pointer_leave(&mut self) {
        self.session.pointer_leave();
    }

    /// Scroll the rows to `y` pixels from the top (the task table's
    /// `scrollTop`), clamped to the schedule.
    pub fn set_scroll_y(&mut self, y: f64) {
        if y.is_finite() {
            let dy = y - self.session.timeline().viewport.scroll_y;
            self.session.scroll_by(dy);
        }
    }

    /// The task table's sideways scrollbar is `px` tall (its `offsetHeight - clientHeight`): the
    /// rows scroll that much further, so the last row clears the bar.
    pub fn set_scroll_slack(&mut self, px: f64) {
        if px.is_finite() {
            self.session.set_scroll_slack(px);
        }
    }

    /// Highlight `row` as hovered while the pointer is outside the canvas (the
    /// task table's hovered row); `undefined` clears it. The canvas pointer
    /// wins while it is over the canvas. `Status.hoverRow` is the row drawn.
    pub fn set_hover_row(&mut self, row: Option<u32>) {
        self.session.set_hover_row(row.map(|row| row as usize));
    }

    /// Early start (minutes) of the task on `row`, if the row exists.
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
    )]
    pub fn row_start(&self, row: u32) -> Option<f64> {
        let t = self.session.timeline().task_at_row(row as usize)?;
        Some(self.session.solution().dates[t as usize].es as f64)
    }

    /// Instant (minutes from the origin) at canvas x.
    pub fn time_at(&self, x: f64) -> f64 {
        self.session.timeline().viewport.t_of(x)
    }

    /// Canvas x of instant `t`.
    pub fn x_of(&self, t: f64) -> f64 {
        self.session.timeline().viewport.x_of(t)
    }
}
