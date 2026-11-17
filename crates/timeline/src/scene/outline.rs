//! The move outline: a dashed ring at the old place of each task an edit or a drag moved
//! (gs#428, story S4 of #309). One [`Rect`] per outline with `stroke > 0`; the shader draws the
//! ring and the dashes inside the same pipeline as the bars.

use super::bars::DIAMOND;
use super::{Builder, Rect, Scene, bar_height, bar_top, span_extent, splice_under_bars};
use crate::Theme;
use crate::summary::Summaries;
use crate::viewport::Viewport;
use engine::Instant;

/// The most outlines one frame draws: the ones nearest the centre of the canvas. A guard, not a
/// case a user meets: it takes a canvas thousands of pixels tall to hold this many rows.
pub const CAP: usize = 400;

/// Ring width, CSS pixels. Provisional, tuned by eye like the other timings of #309.
pub const STROKE_PX: f64 = 1.5;

/// A dash period is this many strokes wide (the shader's `5.0 * stroke`).
const PERIOD_STROKES: f64 = 5.0;

/// The share of the dash colour's alpha: `sel_edge` at 60%.
const DASH_ALPHA: f32 = 0.6;

/// One kind of ring: over the bars or under them, ring width in CSS pixels, share of the alpha.
struct Style(bool, f64, f32);

/// The outline a moved task leaves: under the bars, thin and faint.
const MOVED: Style = Style(false, STROKE_PX, DASH_ALPHA);

/// A proposed edit's ghost: over the bars (a shortened task's ghost lies inside its own bar), a
/// little wider than the moved outline but faint, so at the default zoom it does not shout
/// (gantt#1772).
const GHOST: Style = Style(true, 2.0, 0.7);

/// One outline to draw: its row, squared distance of the bar centre from the canvas centre,
/// and extent in CSS pixels.
struct Candidate {
    task: u32,
    row: usize,
    far: f64,
    x0: f64,
    x1: f64,
    /// A zero-length task: its ring is a diamond, like its bar.
    milestone: bool,
}

/// Add a dashed outline at the old extent of each moved task that has a bar on the canvas, under
/// the bars. `was` lists `(task, old early start, old early finish)`; `row_of` gives a task's row
/// (`None` for a hidden task). A summary row, a row off the canvas, a bar off the canvas or under
/// the axis band, and a task with a hidden row get none. At `alpha <= 0` nothing is added. At most
/// [`CAP`] are drawn, the ones nearest the centre of the canvas. Every outline has the plain bar
/// height, also on a critical row: it marks the old place, not the old look.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
pub fn add_move_outlines(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    summaries: &Summaries,
    row_of: impl Fn(u32) -> Option<usize>,
    was: impl Iterator<Item = (u32, Instant, Instant)>,
    alpha: f64,
    theme: &Theme,
) {
    add_rings(scene, v, dpr, summaries, row_of, was, alpha, theme, &MOVED);
}

/// Add the dashed ghost of each proposed edit, with the rules of [`add_move_outlines`] but above
/// the bars, 2 px wide and at 70% alpha. `at` lists `(task, new early start, new early finish)`.
/// Returns each ring drawn as `(task, right edge in CSS pixels)`, so a name can sit beside it.
#[expect(
    clippy::too_many_arguments,
    reason = "the same inputs as add_move_outlines, whose callers and tests pass them one by one"
)]
pub fn add_ghost_outlines(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    summaries: &Summaries,
    row_of: impl Fn(u32) -> Option<usize>,
    at: impl Iterator<Item = (u32, Instant, Instant)>,
    alpha: f64,
    theme: &Theme,
) -> Vec<(u32, f64)> {
    add_rings(scene, v, dpr, summaries, row_of, at, alpha, theme, &GHOST)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the inputs of add_move_outlines and the style"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "alpha is a fade share, 0 to 1, which f32 holds far finer than a colour channel needs"
)]
fn add_rings(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    summaries: &Summaries,
    row_of: impl Fn(u32) -> Option<usize>,
    was: impl Iterator<Item = (u32, Instant, Instant)>,
    alpha: f64,
    theme: &Theme,
    &Style(over_bars, stroke, share): &Style,
) -> Vec<(u32, f64)> {
    if alpha <= 0.0 {
        return Vec::new();
    }
    let rows = v.visible_rows();
    let bh = bar_height(v);
    let (cx, cy) = (v.width / 2.0, f64::midpoint(v.axis_height, v.height));
    let period = PERIOD_STROKES * stroke;
    let mut found: Vec<Candidate> = Vec::new();
    for (task, es, ef) in was {
        let Some(row) = row_of(task).filter(|r| rows.contains(r)) else {
            continue;
        };
        if summaries.get(task).is_some() {
            continue;
        }
        let top = bar_top(v, row);
        if top < v.axis_height {
            continue;
        }
        let (x0, x1) = span_extent(v, es, ef);
        if x1 < 0.0 || x0 > v.width {
            continue;
        }
        let (dx, dy) = (f64::midpoint(x0, x1) - cx, top + bh / 2.0 - cy);
        found.push(Candidate {
            task,
            row,
            far: dx * dx + dy * dy,
            x0,
            x1,
            milestone: es == ef,
        });
    }
    let by_distance =
        |a: &Candidate, b: &Candidate| a.far.total_cmp(&b.far).then(a.row.cmp(&b.row));
    if found.len() > CAP {
        found.select_nth_unstable_by(CAP - 1, by_distance);
        found.truncate(CAP);
    }
    found.sort_unstable_by_key(|c| c.row);
    let mut b = Builder {
        dpr,
        arrow_color: theme.link,
        scene: Scene::default(),
    };
    let mut color = theme.sel_edge;
    color[3] *= share * alpha as f32;
    let mut drawn = Vec::with_capacity(found.len());
    for c in found {
        drawn.push((c.task, c.x1));
        let (mut x0, mut x1) = (c.x0, c.x1);
        // A diamond keeps its whole box, a few pixels wide: cut at the canvas edge it would draw
        // lopsided (the GPU clips what lies past the canvas).
        if !c.milestone {
            x1 = x1.min(v.width + 2.0);
            if x0 < -2.0 {
                // Keep the dash phase fixed to the bar while it scrolls: start the clipped quad a
                // whole number of periods after the real start, still left of the canvas.
                x0 += ((-2.0 - x0) / period).floor() * period;
            }
        }
        let top = bar_top(v, c.row);
        let radius = if c.milestone { DIAMOND } else { v.bar_radius };
        b.stroked(x0, top, x1, top + bh, color, radius, stroke);
    }
    let rects: Vec<Rect> = b.scene.rects;
    if over_bars {
        scene.rects.extend(rects);
    } else {
        splice_under_bars(scene, rects);
    }
    drawn
}
