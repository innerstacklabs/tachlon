//! One frame's draw list, built on the CPU from the viewport and the task
//! dates: rectangles (bars, grid, axis band) and axis labels.
//!
//! Positions are computed in `f64` CSS pixels and converted to `f32`
//! physical pixels only at the end, after culling, so no `f32` ever holds a
//! large coordinate. Only rows in the row window produce bars.
//!
//! Dependency arrows are a second pass over the visible range only
//! ([`add_arrows`]): a [`LinkIndex`] finds the links whose rows meet the
//! row window, and `link_in_view` keeps those whose path meets the canvas.

use crate::Theme;
use crate::axis;
use crate::labels::BarLabel;
use crate::viewport::Viewport;
use engine::{Calendar, Instant, Task, TaskDates};

mod backdrop;
pub(crate) mod bar_color;
mod bars;
mod baseline;
mod constraint;
mod header;
mod link_hit;
mod link_path;
mod link_preview;
mod links;
mod long_links;
mod outline;
mod svg;

pub use bar_color::BarColors;
pub use bars::{build, build_marked};
pub use baseline::add_comparison_bars;
pub use constraint::StartConstraints;
pub use link_hit::HIT_SLOP;
pub use link_path::{HEAD, STUB};
pub use link_preview::{
    PreviewFrom, PreviewTo, add_link_preview, add_link_terminal, terminal_contains,
};
pub use links::{Link, LinkIndex, Placed, add_arrows, add_arrows_highlighted};
pub use outline::{CAP as OUTLINE_CAP, STROKE_PX, add_ghost_outlines, add_move_outlines};
pub use svg::svg;

/// One instanced quad, physical pixels, sRGB colour with alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rect {
    /// Top-left corner.
    pub pos: [f32; 2],
    /// Width and height.
    pub size: [f32; 2],
    /// sRGB colour, straight alpha.
    pub color: [f32; 4],
    /// Corner radius in physical pixels; the shader rounds the corners with a
    /// signed distance field and clamps it to half the shorter side. 0 is square. Below 0 is a
    /// marker, not a size: -1 a diamond, -2 a triangle with its tip on the right, -3 on the left.
    pub radius: f32,
    /// Ring width in physical pixels: 0 fills the quad; above 0 the shader draws a dashed ring
    /// of this width just inside the quad's edge (the move outline).
    pub stroke: f32,
}

/// One axis label, physical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// Text.
    pub text: String,
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Clip the label at this x.
    pub right: f32,
    /// Font size, physical pixels.
    pub px: f32,
    /// Text colour; `None` is `Theme::text`.
    pub color: Option<[f32; 4]>,
    /// Centre the text between `x` and `right`; false sets it at `x`.
    pub center: bool,
}

/// A frame's draw list, including the task and rectangle index of each bar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    /// Quads in draw order.
    pub rects: Vec<Rect>,
    /// Axis labels.
    pub labels: Vec<Label>,
    /// Number of task bars among `rects`.
    pub bars: usize,
    /// Number of summary bars among `rects`; not counted in `bars`.
    pub summary_bars: usize,
    /// Number of critical bars among them.
    pub critical_bars: usize,
    /// Number of dependency arrows drawn (see [`add_arrows`]).
    pub arrows: usize,
    /// Task id and index in `rects` of each task bar rectangle.
    pub bar_rects: Vec<(u32, usize)>,
    /// Task id and index in `rects` of each further part of a bar: a critical
    /// bar's edge and a summary bracket's two end ticks. They take their
    /// task's highlight or dimming.
    pub bar_extras: Vec<(u32, usize)>,
    /// Task id and index in `rects` of each bar's progress quad: one per visible bar
    /// with some progress, never one per task. It takes its task's highlight or dimming.
    pub bar_progress: Vec<(u32, usize)>,
    /// Task id and host color of each bar drawn in one (see [`BarColors`]).
    pub bar_colored: Vec<(u32, [f32; 4])>,
    /// Number of non-working-day column rectangles (see [`Backdrop`]).
    pub shaded: usize,
    /// x of the data-date line in CSS pixels, when it is on the canvas.
    pub data_date_x: Option<f64>,
    /// Index in `rects` of the first bar: arrows go in just before it, so
    /// bars draw over them.
    pub bar_start: usize,
    /// Task names on or beside bars, placed by [`crate::labels::add_bar_labels`].
    pub bar_labels: Vec<BarLabel>,
}

/// Alpha factor of everything that is not highlighted.
pub const DIM: f32 = 0.4;

/// Non-working days are shaded only when a day is wider than this, CSS pixels.
pub const MIN_SHADE_DAY_PX: f64 = 3.0;
/// Height of a summary bracket, CSS pixels.
const BRACKET_H: f64 = 7.0;
/// Thickness of a summary bracket's bar, CSS pixels.
const BRACKET_BAR: f64 = 3.0;
/// Width of a summary bracket's end tick, CSS pixels.
const BRACKET_TICK: f64 = 2.0;
/// The data-date pill's text.
const TAG_TEXT: &str = "Data date";
/// Minimum bar width, CSS pixels, so a short task stays visible zoomed out.
pub const MIN_BAR_PX: f64 = 2.0;

/// The tasks and dependency links of a highlighted path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlight {
    tasks: Vec<bool>,
    edges: std::collections::HashSet<(u32, u32)>,
    /// A partial path ([`Highlight::unlit`]): a link counts only while both its ends are lit.
    gated: bool,
}

impl Highlight {
    /// Build a highlight, ignoring task ids outside `task_count`.
    pub fn new(
        task_count: usize,
        tasks: impl IntoIterator<Item = u32>,
        edges: impl IntoIterator<Item = (u32, u32)>,
    ) -> Highlight {
        let mut selected = vec![false; task_count];
        for task in tasks {
            if let Some(slot) = selected.get_mut(task as usize) {
                *slot = true;
            }
        }
        Highlight {
            tasks: selected,
            edges: edges.into_iter().collect(),
            gated: false,
        }
    }

    /// Whether `task` belongs to the highlighted path.
    #[must_use]
    pub fn has_task(&self, task: u32) -> bool {
        self.tasks.get(task as usize).copied().unwrap_or(false)
    }

    /// This highlight with every task in `ranked` unlit, to be lit again one at a time with
    /// [`Highlight::light`] (the first `n` ranks of a path). Tasks not in `ranked` stay lit, and a
    /// link counts only while both its ends are lit.
    #[must_use]
    pub fn unlit(&self, ranked: impl IntoIterator<Item = u32>) -> Highlight {
        let mut out = self.clone();
        out.gated = true;
        for task in ranked {
            if let Some(slot) = out.tasks.get_mut(task as usize) {
                *slot = false;
            }
        }
        out
    }

    /// Light `task` again; its links to lit tasks follow.
    pub fn light(&mut self, task: u32) {
        if let Some(slot) = self.tasks.get_mut(task as usize) {
            *slot = true;
        }
    }

    /// Whether the dependency from `pred` to `succ` belongs to the path.
    #[must_use]
    pub fn has_edge(&self, pred: u32, succ: u32) -> bool {
        self.edges.contains(&(pred, succ))
            && (!self.gated || (self.has_task(pred) && self.has_task(succ)))
    }
}

/// Colour the highlighted task bars and dim every other task bar.
///
/// A highlighted bar is solid `sel_edge`; its progress quad becomes the
/// `bar_sel_progress` token, alpha included, so the progress still reads on the accent.
pub fn apply_highlight(scene: &mut Scene, highlight: &Highlight, theme: &Theme) {
    let progress = scene
        .bar_progress
        .iter()
        .map(|&(task, index)| (task, index, true));
    let rest = scene
        .bar_rects
        .iter()
        .chain(&scene.bar_extras)
        .map(|&(task, index)| (task, index, false));
    for (task, index, is_progress) in progress.chain(rest) {
        let color = &mut scene.rects[index].color;
        if !highlight.has_task(task) {
            color[3] *= DIM;
        } else if is_progress {
            *color = theme.bar_sel_progress;
        } else {
            *color = theme.sel_edge;
        }
    }
}

/// Horizontal extent of a bar, CSS pixels: `[x0, x1)`. Milestones (zero
/// duration) are a square centred on their date.
#[must_use]
pub fn bar_extent(v: &Viewport, d: &TaskDates) -> (f64, f64) {
    span_extent(v, d.es, d.ef)
}

/// Horizontal extent of a time span, CSS pixels: `[x0, x1)`. A zero-length
/// span is a square centred on its date.
#[expect(
    clippy::cast_precision_loss,
    reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
)]
#[must_use]
pub fn span_extent(v: &Viewport, start: Instant, end: Instant) -> (f64, f64) {
    let x0 = v.x_of(start as f64);
    if end == start {
        let h = bar_height(v);
        return (x0 - h / 2.0, x0 + h / 2.0);
    }
    let x1 = v.x_of(end as f64);
    (x0, x1.max(x0 + MIN_BAR_PX))
}

pub(crate) fn bar_height(v: &Viewport) -> f64 {
    (v.row_height * v.bar_ratio).round()
}

/// Top of the bar on `row`, CSS pixels.
pub(crate) fn bar_top(v: &Viewport, row: usize) -> f64 {
    v.row_top(row) + ((v.row_height - bar_height(v)) / 2.0).round()
}

/// Put `rects` in the draw list just before the first bar (`bar_start`), under the bars, and move
/// the bar indexes past them. Rects spliced earlier at the same place end up above these.
fn splice_under_bars(scene: &mut Scene, rects: Vec<Rect>) {
    let at = scene.bar_start.min(scene.rects.len());
    let added = rects.len();
    scene.rects.splice(at..at, rects);
    let moved = scene.bar_rects.iter_mut().chain(&mut scene.bar_extras);
    for (_, index) in moved.chain(&mut scene.bar_progress) {
        if *index >= at {
            *index += added;
        }
    }
}

struct Builder {
    dpr: f64,
    arrow_color: [f32; 4],
    scene: Scene,
}

impl Builder {
    /// Push a square rectangle given in CSS pixels, snapped to physical pixels.
    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: [f32; 4]) {
        self.rounded(x0, y0, x1, y1, color, 0.0);
    }

    /// [`Builder::rect`] with corners of `radius` CSS pixels.
    fn rounded(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: [f32; 4], radius: f64) {
        self.stroked(x0, y0, x1, y1, color, radius, 0.0);
    }

    /// [`Builder::rounded`] drawn as a dashed ring `stroke` CSS pixels wide; 0 fills it.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four corners, the colour, the radius and the stroke are plain numbers, and no other call shares them"
    )]
    fn stroked(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: [f32; 4],
        radius: f64,
        stroke: f64,
    ) {
        let s = self.dpr;
        let (px0, py0) = ((x0 * s).round(), (y0 * s).round());
        let (px1, py1) = (
            (x1 * s).round().max(px0 + 1.0),
            (y1 * s).round().max(py0 + 1.0),
        );
        #[expect(
            clippy::cast_possible_truncation,
            reason = "f32 is the vertex type; a value on the canvas (under 2^24 physical pixels) keeps sub-pixel accuracy, and one far off the canvas is not seen"
        )]
        self.scene.rects.push(Rect {
            pos: [px0 as f32, py0 as f32],
            size: [(px1 - px0) as f32, (py1 - py0) as f32],
            color,
            radius: (radius * if radius < 0.0 { 1.0 } else { s }) as f32,
            stroke: (stroke * s) as f32,
        });
    }

    /// A one-physical-pixel vertical line at CSS x.
    fn vline(&mut self, x: f64, y0: f64, y1: f64, color: [f32; 4]) {
        self.rect(x, y0, x + 1.0 / self.dpr, y1, color);
    }

    /// An axis label between CSS x `span.0` and `span.1`, centred there or set at its left end.
    fn label(
        &mut self,
        text: String,
        span: (f64, f64),
        y: f64,
        px: f64,
        color: Option<[f32; 4]>,
        center: bool,
    ) {
        let s = self.dpr;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "f32 is the vertex type; a value on the canvas (under 2^24 physical pixels) keeps sub-pixel accuracy, and one far off the canvas is not seen"
        )]
        self.scene.labels.push(Label {
            text,
            x: (span.0 * s).round() as f32,
            y: (y * s).round() as f32,
            right: (span.1 * s).round() as f32,
            px: (px * s) as f32,
            color,
            center,
        });
    }
}

/// What the scene draws from the schedule beyond the task dates.
#[derive(Debug, Clone, Copy, Default)]
pub struct Backdrop<'a> {
    /// The calendar whose non-working days are shaded as columns under the
    /// bars; `None` shades nothing.
    pub calendar: Option<&'a Calendar>,
    /// The data date, drawn as a line with a tag on the axis.
    pub data_date: Option<Instant>,
    /// The calendar date of instant 0, for the year, month and day labels and where month
    /// and year ticks fall (gs#150). Defaults to [`axis::Origin::DEMO`].
    pub origin: axis::Origin,
    /// The schedule's tasks, indexed by task id, for their actuals. A started task's progress
    /// quad runs from its actual start to the data date; a finished task's fills its bar. A task
    /// past the slice, or `None`, has no progress quad.
    pub progress: Option<&'a [Task]>,
    /// The deadline of each task, indexed by task id (display only; no date depends on it). A bar
    /// whose early finish falls after its deadline gets a mark at the deadline. A task past the
    /// slice, or `None`, has no mark.
    pub deadlines: Option<&'a [Option<Instant>]>,
    /// Which tasks hold a real start constraint (display only; no date depends on it). A bar that
    /// does gets a mark on its left edge. `None` marks nothing.
    pub start_constraints: Option<&'a dyn StartConstraints>,
    /// The color each task's bar takes (display only). A critical bar and a task with no color
    /// draw as the theme does. `None` colors nothing.
    pub bar_colors: Option<&'a dyn BarColors>,
}

/// The rows the scene tints beyond the stripes. The task table tints the same
/// rows with the same tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowMarks {
    /// The hovered row (`Theme::hov`).
    pub hover: Option<usize>,
    /// The selected task's row (`Theme::sel`).
    pub selected: Option<usize>,
    /// The range selection's rows, first to last inclusive (`Theme::sel`, the selected row's fill).
    /// Only the rows in view are drawn, so a long range costs the window, not the range.
    pub range: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests;
