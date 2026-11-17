//! The Gantt timeline renderer (milestone M2).
//!
//! [`Timeline`] holds the viewport and the pointer state and turns input
//! into viewport changes; [`scene::build`] turns the viewport and the task
//! dates into a draw list; [`render::Renderer`] draws that list with wgpu
//! (instanced quads) and glyphon (axis labels). Everything except the
//! renderer is plain Rust with no GPU, so it is unit-tested natively.

#![cfg_attr(not(test), warn(clippy::too_many_lines))]

pub mod axis;
mod hover;
mod jump;
pub mod labels;
pub mod names;
pub mod render;
pub mod scene;
pub mod slide;
pub mod summary;
mod text;
pub mod theme;
mod view;
pub mod viewport;

pub use scene::Scene;
pub use summary::Summaries;
pub use theme::Theme;
pub use viewport::Viewport;

use engine::TaskDates;

/// Wheel zoom: scale factor per pixel of wheel delta.
pub const ZOOM_PER_PX: f64 = 0.002;

/// Width of the zone at a bar's finish edge that counts as the edge, CSS pixels.
pub const EDGE_PX: f64 = 6.0;

/// Sentinel row for a task that is not in the current row order.
pub const HIDDEN_ROW: u32 = u32::MAX;

/// A bar drag in progress: which task, and where on the bar it was grabbed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDrag {
    /// The task being moved.
    pub task: u32,
    /// Minutes from the bar's early start to the grab point.
    pub grab: f64,
    /// Pointer x, CSS pixels.
    pub x: f64,
    /// The press was on the bar's finish edge, not its body.
    pub edge: bool,
}

impl BarDrag {
    /// The start the pointer asks for, in whole minutes.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the start is minutes from the schedule origin, far inside i64"
    )]
    #[must_use]
    pub fn wanted_start(&self, viewport: &Viewport) -> i64 {
        (viewport.t_of(self.x) - self.grab).round() as i64
    }
}

/// Viewport and pointer state for one canvas.
#[derive(Debug, Clone)]
pub struct Timeline {
    /// What the canvas shows.
    pub viewport: Viewport,
    /// Device pixel ratio.
    pub dpr: f64,
    /// `order[r]` is the task on row r.
    order: Vec<u32>,
    /// `row_of[t]` is task t's row on screen, or [`HIDDEN_ROW`] when it has no row.
    row_of: Vec<u32>,
    /// The rows on screen while the list is grouped, [`HIDDEN_ROW`] on a header; empty otherwise.
    view: Vec<u32>,
    /// Last pointer position while a pan drag is in progress.
    drag: Option<(f64, f64)>,
    /// The bar being dragged, if a drag started on a bar.
    bar_drag: Option<BarDrag>,
    /// Last pointer position over the canvas.
    pointer: Option<(f64, f64)>,
    /// A row the shell asks to highlight while the pointer is not over the
    /// canvas (the task table's hovered row).
    outside_hover: Option<usize>,
    /// Row of the last jump to a critical task.
    last_jump: Option<usize>,
    /// Summary-task spans, indexed by task id.
    summaries: Summaries,
    /// Percent complete per task (0 to 100), indexed by task id; empty draws no progress.
    progress: Vec<u8>,
}

impl Timeline {
    /// A timeline of `width` x `height` CSS pixels showing tasks in row order
    /// `order`. The task-to-row table covers every task that has a row.
    #[must_use]
    pub fn new(order: Vec<u32>, width: f64, height: f64, dpr: f64) -> Timeline {
        Timeline {
            viewport: Viewport::new(width, height, order.len()),
            dpr: dpr.max(0.25),
            row_of: view::row_index(&order, 0),
            order,
            view: Vec::new(),
            drag: None,
            bar_drag: None,
            pointer: None,
            outside_hover: None,
            last_jump: None,
            summaries: Summaries::default(),
            progress: Vec::new(),
        }
    }

    /// Percent complete per task, indexed by task id.
    #[must_use]
    pub fn progress(&self) -> &[u8] {
        &self.progress
    }

    /// Replace the percent complete per task; the scene draws a progress quad on each bar that has some.
    pub fn set_progress(&mut self, progress: Vec<u8>) {
        self.progress = progress;
    }

    /// Summary-task spans, indexed by task id.
    #[must_use]
    pub fn summaries(&self) -> &Summaries {
        &self.summaries
    }

    /// Mutable summary-task spans, written only by `app::Session`.
    pub fn summaries_mut(&mut self) -> &mut Summaries {
        &mut self.summaries
    }

    /// Row order: `order()[r]` is the task on row r.
    #[must_use]
    pub fn order(&self) -> &[u32] {
        &self.order
    }

    /// Task-to-row index. [`HIDDEN_ROW`] means the task has no current row.
    #[must_use]
    pub fn row_of(&self) -> &[u32] {
        &self.row_of
    }

    /// The current row of `task`, or `None` when it is hidden or unknown.
    #[must_use]
    pub fn row_of_task(&self, task: u32) -> Option<usize> {
        self.row_of
            .get(task as usize)
            .copied()
            .filter(|&row| row != HIDDEN_ROW)
            .map(|row| row as usize)
    }

    /// Replace the row order and rebuild the task-to-row index.
    ///
    /// A caller must not change the order during a bar drag; [`Timeline`]
    /// deliberately leaves its pointer, pan and drag state untouched.
    pub fn set_order(&mut self, order: Vec<u32>) {
        self.row_of = view::row_index(&order, self.row_of.len());
        self.order = order;
        self.view.clear();
        self.viewport.rows = self.order.len();
        self.viewport.scroll_y = self
            .viewport
            .scroll_y
            .clamp(0.0, self.viewport.max_scroll());
        self.last_jump = None;
    }

    /// Resize to `width` x `height` CSS pixels at `dpr`.
    pub fn resize(&mut self, width: f64, height: f64, dpr: f64) {
        self.dpr = dpr.max(0.25);
        self.viewport.set_size(width, height);
    }

    /// The table's sideways scrollbar is `px` tall: the rows scroll that much further.
    pub fn set_scroll_slack(&mut self, px: f64) {
        self.viewport.scroll_slack = px.max(0.0);
        self.viewport.scroll_y = self
            .viewport
            .scroll_y
            .clamp(0.0, self.viewport.max_scroll());
    }

    /// A wheel event at (x, y). `zoom` (ctrl or pinch) zooms anchored at x;
    /// otherwise the wheel scrolls rows (dy) and pans time (dx). Ignored
    /// during a bar drag: the view would move under a still pointer, which
    /// the drag would read as a new start.
    pub fn wheel(&mut self, x: f64, _y: f64, dx: f64, dy: f64, zoom: bool) {
        if self.bar_drag.is_some() {
            return;
        }
        if zoom {
            self.viewport.zoom_at(x, (-dy * ZOOM_PER_PX).exp());
        } else {
            self.viewport.pan(-dx, 0.0);
            self.viewport.scroll_by(dy);
        }
    }

    /// Pointer pressed at (x, y): on a bar, starts dragging that bar;
    /// elsewhere, starts a pan drag.
    pub fn pointer_down(&mut self, dates: &[TaskDates], x: f64, y: f64) {
        self.pointer = Some((x, y));
        self.drag = None;
        self.bar_drag = None;
        if let Some(task) = self.hit_test(dates, x, y) {
            if self.summaries.get(task).is_some() {
                self.drag = Some((x, y));
            } else {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
                )]
                let grab = self.viewport.t_of(x) - dates[task as usize].es as f64;
                let edge = self.hit_edge(dates, x, y);
                self.bar_drag = Some(BarDrag {
                    task,
                    grab,
                    x,
                    edge,
                });
            }
        } else {
            self.drag = Some((x, y));
        }
    }

    /// Pointer moved to (x, y): pans during a pan drag, moves the target
    /// during a bar drag.
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        if let Some((lx, ly)) = self.drag {
            self.viewport.pan(x - lx, y - ly);
            self.drag = Some((x, y));
        }
        if let Some(b) = &mut self.bar_drag {
            b.x = x;
        }
        self.pointer = Some((x, y));
    }

    /// Pointer released (or cancelled, or left the canvas).
    pub fn pointer_up(&mut self) {
        self.drag = None;
        self.bar_drag = None;
    }

    /// Pointer left the canvas. A bar drag keeps going under pointer
    /// capture, so only a pan drag ends here.
    pub fn pointer_leave(&mut self) {
        self.drag = None;
        if self.bar_drag.is_none() {
            self.pointer = None;
        }
    }

    /// Whether a pan drag is in progress.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The bar drag in progress, if any.
    #[must_use]
    pub fn bar_drag(&self) -> Option<BarDrag> {
        self.bar_drag
    }

    /// Hit test at (x, y): the row under y, then the bar on that row under x.
    #[must_use]
    pub fn hit_test(&self, dates: &[TaskDates], x: f64, y: f64) -> Option<u32> {
        let row = self.viewport.row_at(y)?;
        let task = *self.rows().get(row).filter(|&&task| task != HIDDEN_ROW)?;
        let (x0, x1) = self.extent(task, dates);
        (x0 <= x && x < x1).then_some(task)
    }

    /// Whether (x, y) is on a leaf bar's finish edge: the last [`EDGE_PX`] of the bar, or its last
    /// third when the bar is narrower than three times that. A milestone, a summary and a bar
    /// at the [`scene::MIN_BAR_PX`] floor have no edge: there is no body to tell it from.
    #[must_use]
    pub fn hit_edge(&self, dates: &[TaskDates], x: f64, y: f64) -> bool {
        let Some(task) = self.hit_test(dates, x, y) else {
            return false;
        };
        let d = &dates[task as usize];
        if self.summaries.get(task).is_some() || d.es == d.ef {
            return false;
        }
        let (x0, x1) = self.extent(task, dates);
        x1 - x0 > scene::MIN_BAR_PX && x >= x1 - EDGE_PX.min((x1 - x0) / 3.0)
    }

    fn extent(&self, task: u32, dates: &[TaskDates]) -> (f64, f64) {
        self.summaries.get(task).map_or_else(
            || scene::bar_extent(&self.viewport, &dates[task as usize]),
            |(start, end)| scene::span_extent(&self.viewport, start, end),
        )
    }

    /// Highlight `row` as if the pointer were on it, until `None` clears it.
    /// This is for a pointer outside the canvas (the task table); the canvas
    /// pointer wins while it is over the canvas.
    pub fn set_outside_hover(&mut self, row: Option<usize>) {
        self.outside_hover = row;
    }

    /// The row the scene highlights as hovered: the row under the canvas
    /// pointer, else the row set by [`Self::set_outside_hover`] if it still
    /// exists.
    #[must_use]
    pub fn hover_row(&self) -> Option<usize> {
        self.pointer
            .and_then(|(_, y)| self.viewport.row_at(y))
            .or(self.outside_hover.filter(|&row| row < self.rows().len()))
    }

    /// Task under the pointer, if any.
    #[must_use]
    pub fn hovered(&self, dates: &[TaskDates]) -> Option<u32> {
        let (x, y) = self.pointer?;
        self.hit_test(dates, x, y)
    }

    /// This frame's draw list.
    /// `held`: the dragged task's predecessors hold it later than asked.
    /// `selected` is the selected task's row, tinted like the table's.
    /// `range` is the range selection's first and last row, tinted the same.
    /// `backdrop` is the calendar to shade and the data date to mark.
    #[must_use]
    pub fn scene(
        &self,
        dates: &[TaskDates],
        held: bool,
        selected: Option<usize>,
        range: Option<(usize, usize)>,
        backdrop: scene::Backdrop<'_>,
        theme: &Theme,
    ) -> Scene {
        let dragged = self.bar_drag.map(|b| (b.task, held));
        scene::build_marked(
            &self.viewport,
            self.dpr,
            self.rows(),
            dates,
            &self.summaries,
            scene::RowMarks {
                hover: self.hover_row(),
                selected,
                range,
            },
            dragged,
            backdrop,
            theme,
        )
    }
}

/// Row order for a solution: by early start, then task id.
#[must_use]
pub fn order_by_start(dates: &[TaskDates]) -> Vec<u32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "task ids are u32, so a task count fits u32"
    )]
    let mut order: Vec<u32> = (0..dates.len() as u32).collect();
    order.sort_by_key(|&t| (dates[t as usize].es, t));
    order
}

#[cfg(test)]
mod tests;
