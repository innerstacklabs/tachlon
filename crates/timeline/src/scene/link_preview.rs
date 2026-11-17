//! The link being dragged out, and the terminal it is dragged from. Both are
//! a handful of rectangles drawn after the bars, whatever the schedule size.

use super::link_path::{Seg, path};
use super::links::{Clip, Link, Placed};
use super::{Builder, Scene, bar_extent};
use crate::Theme;
use crate::viewport::Viewport;
use engine::{LinkType, TaskDates};

/// How far outside the terminal's dot a press still lands on it, CSS pixels.
const TERMINAL_SLOP: f64 = 2.0;

/// Where a link drag starts: the predecessor's row and the x of its finish.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewFrom {
    /// Row of the predecessor.
    pub row: usize,
    /// x of the predecessor's finish end, CSS pixels.
    pub x: f64,
}

/// Where the preview line ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewTo {
    /// Free in the chart: the pointer, CSS pixels.
    Point(f64, f64),
    /// On a target bar: its row and the x of its start, CSS pixels.
    Bar(usize, f64),
}

fn builder(dpr: f64, color: [f32; 4]) -> Builder {
    Builder {
        dpr,
        arrow_color: color,
        scene: Scene::default(),
    }
}

/// Draw the link being dragged from `from` to `to`, over the bars: the path
/// a finish-to-start link would take (to a bar), or an elbow to the pointer.
/// `refused` turns it `link_refused` before release. `width` is the line's
/// thickness (`--gantt-preview-w`). Two to nine rectangles.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
pub fn add_link_preview(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    from: PreviewFrom,
    to: PreviewTo,
    refused: bool,
    width: f64,
    theme: &Theme,
) {
    let mut b = builder(
        dpr,
        if refused {
            theme.link_refused
        } else {
            theme.sel_edge
        },
    );
    let clip = Clip {
        x0: -2.0,
        x1: v.width + 2.0,
        y0: v.axis_height,
        y1: v.height,
    };
    let ya = v.row_top(from.row) + v.row_height / 2.0;
    match to {
        PreviewTo::Bar(row, x) if row != from.row => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a row is an index into the order, which lists tasks by their u32 ids, so it fits u32"
            )]
            let link = Link {
                dep: 0,
                pred: 0,
                succ: 0,
                pred_row: from.row as u32,
                succ_row: row as u32,
                kind: LinkType::FS,
            };
            let route = path(
                v,
                &Placed {
                    link,
                    xa: from.x,
                    xb: x,
                },
            );
            for seg in route.segs {
                match seg {
                    Seg::H(xa, xb, y) => b.hseg(xa, xb, y, width, &clip),
                    Seg::V(x, ya, yb) => b.vseg(x, ya, yb, width, &clip),
                }
            }
            b.head(route.head.0, route.head.1, route.head.2, &clip);
        }
        PreviewTo::Bar(_, x) => b.hseg(from.x, x, ya, width, &clip),
        PreviewTo::Point(x, y) => {
            b.hseg(from.x, x, ya, width, &clip);
            b.vseg(x, ya, y, width, &clip);
        }
    }
    scene.rects.append(&mut b.scene.rects);
}

/// Draw the link terminal outside the finish end of the bar of `dates` on
/// `row`: a round `size`-pixel dot in `link_terminal`, `gap` pixels clear of
/// the bar (`--gantt-terminal-size`, `--gantt-terminal-gap`), for the hovered
/// or selected bar only. One rectangle; nothing when the row or the end is
/// off the canvas.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
pub fn add_link_terminal(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    row: usize,
    dates: &TaskDates,
    size: f64,
    gap: f64,
    theme: &Theme,
) {
    let x = bar_extent(v, dates).1 + gap;
    let y = v.row_top(row) + v.row_height / 2.0;
    if x + size < 0.0 || x > v.width || y - size / 2.0 < v.axis_height || y + size / 2.0 > v.height
    {
        return;
    }
    let mut b = builder(dpr, theme.link_terminal);
    b.rounded(
        x,
        y - size / 2.0,
        x + size,
        y + size / 2.0,
        theme.link_terminal,
        size / 2.0,
    );
    scene.rects.append(&mut b.scene.rects);
}

/// Whether (x, y) presses the terminal that [`add_link_terminal`] draws for the bar of `dates`
/// on `row`, `size` and `gap` as `add_link_terminal` takes them: the dot and 2 pixels around it. The
/// same geometry, so a press lands where the dot is drawn.
#[must_use]
pub fn terminal_contains(
    v: &Viewport,
    row: usize,
    dates: &TaskDates,
    size: f64,
    gap: f64,
    (x, y): (f64, f64),
) -> bool {
    let left = bar_extent(v, dates).1 + gap;
    let mid = v.row_top(row) + v.row_height / 2.0;
    (left - TERMINAL_SLOP..=left + size + TERMINAL_SLOP).contains(&x)
        && (y - mid).abs() <= size / 2.0 + TERMINAL_SLOP
}
