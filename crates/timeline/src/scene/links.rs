//! Dependency links: placing them on rows, finding the visible ones, drawing the arrows.

use super::link_path::{HEAD, Seg, entry_x, path};
use super::long_links::LongLinks;
use super::{Builder, DIM, Highlight, Scene, bar_extent};
use crate::Theme;
use crate::viewport::Viewport;
use core::ops::Range;
use engine::{DepId, Dependency, LinkType, Schedule, TaskDates};

/// Links that span more rows than this are found through `LongLinks`; the
/// rest by binary search on their upper row, which they leave at most this
/// many rows above the window.
pub(super) const LONG_ROWS: usize = 256;

/// The `Rect::radius` that marks an arrowhead triangle with its tip on the right, and on the
/// left. Like `bars::DIAMOND`, the shader reads a negative radius as a marker, not a size.
pub(super) const TRIANGLE_RIGHT: f64 = -2.0;
pub(super) const TRIANGLE_LEFT: f64 = -3.0;

/// A dependency placed on rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    /// The dependency's id: its slot in the schedule's dependency list.
    pub dep: u32,
    /// Predecessor task.
    pub pred: u32,
    /// Successor task.
    pub succ: u32,
    /// Row of the predecessor.
    pub pred_row: u32,
    /// Row of the successor.
    pub succ_row: u32,
    /// Which end of each bar the link joins.
    pub kind: LinkType,
}

impl Link {
    pub(super) fn lo(&self) -> usize {
        self.pred_row.min(self.succ_row) as usize
    }

    pub(super) fn hi(&self) -> usize {
        self.pred_row.max(self.succ_row) as usize
    }
}

/// A link that meets the view, with the canvas x of its two ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// The link.
    pub link: Link,
    /// x where the path leaves the predecessor, CSS pixels.
    pub xa: f64,
    /// x where the path enters the successor, CSS pixels.
    pub xb: f64,
}

/// The dependencies of a schedule, placed on rows once: rows never change
/// while dates do, so only the x positions are computed per frame.
#[derive(Debug, Clone, Default)]
pub struct LinkIndex {
    /// Links of at most `LONG_ROWS` rows, sorted by their upper row.
    short: Vec<Link>,
    /// Longer links.
    long: LongLinks,
}

impl LinkIndex {
    /// Place `deps` on rows; `order[r]` is the task on row r, or [`crate::HIDDEN_ROW`] on a
    /// group header. `order` can be
    /// shorter than the task count when a collapse hides rows; a task not in
    /// `order` has no row, and a dependency naming it is left out. `deps[i]`
    /// has id `i`; a dependency for which `live` is false (removed, or an end
    /// deleted) is left out too.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row indexes the order and a dependency index is its id, a u32, so both fit u32"
    )]
    pub fn new(deps: &[Dependency], live: impl Fn(DepId) -> bool, order: &[u32]) -> LinkIndex {
        let row_of = crate::view::row_index(order, 0);
        let row = |t: u32| row_of.get(t as usize).copied().filter(|&r| r != u32::MAX);
        let (mut short, mut long) = (Vec::new(), Vec::new());
        for (i, d) in deps.iter().enumerate() {
            if !live(DepId(i as u32)) {
                continue;
            }
            let (Some(pred_row), Some(succ_row)) = (row(d.pred.0), row(d.succ.0)) else {
                continue;
            };
            let link = Link {
                dep: i as u32,
                pred: d.pred.0,
                succ: d.succ.0,
                pred_row,
                succ_row,
                kind: d.kind,
            };
            if link.hi() - link.lo() > LONG_ROWS {
                long.push(link);
            } else {
                short.push(link);
            }
        }
        short.sort_by_key(Link::lo);
        LinkIndex {
            short,
            long: LongLinks::new(long),
        }
    }

    /// [`LinkIndex::new`] over the dependencies in force in `schedule`.
    #[must_use]
    pub fn of(schedule: &Schedule, order: &[u32]) -> LinkIndex {
        LinkIndex::new(
            schedule.dependencies(),
            |d| schedule.is_dependency_live(d),
            order,
        )
    }

    /// Number of links.
    #[must_use]
    pub fn len(&self) -> usize {
        self.short.len() + self.long.len()
    }

    /// Whether there are no links.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The links whose path meets the view, found by looking at the visible
    /// rows only: a short link starts at most `LONG_ROWS` rows above the
    /// window, so a binary search bounds the candidates; the long ones are
    /// found by `LongLinks::candidates`, which looks at the links that meet
    /// the rows only. `link_in_view` decides each candidate; for a long link
    /// that spans the rows, whose path can only show as its vertical run, one
    /// bar's x decides it. The short links come first, in order of upper row,
    /// then the long ones in order of id.
    #[must_use]
    pub fn visible(&self, v: &Viewport, dates: &[TaskDates]) -> Vec<Placed> {
        let rows = v.visible_rows();
        if rows.is_empty() {
            return Vec::new();
        }
        let first = rows.start.saturating_sub(LONG_ROWS);
        let from = self.short.partition_point(|l| l.lo() < first);
        let to = self.short.partition_point(|l| l.lo() < rows.end);
        let mut placed: Vec<Placed> = self.short[from..to]
            .iter()
            // Cheap row test first: about LONG_ROWS candidates end above the window.
            .filter(|l| l.hi() >= rows.start && l.lo() < rows.end)
            .filter_map(|&link| {
                let (xa, xb) = anchors(v, &link, dates);
                link_in_view(v, &rows, &link, xa, xb).then_some(Placed { link, xa, xb })
            })
            .collect();
        let short = placed.len();
        // A run into a bar end shows if its x meets the canvas: the vertical
        // run of a link that spans the rows, whatever its other end does.
        let enters_in_view = |succ: u32, finish: bool| {
            let (s0, s1) = bar_extent(v, &dates[succ as usize]);
            let xi = entry_x(finish, if finish { s1 } else { s0 });
            x_in(v, xi, xi)
        };
        self.long.candidates(&rows, enters_in_view, |&link, spans| {
            let (xa, xb) = anchors(v, &link, dates);
            if spans || link_in_view(v, &rows, &link, xa, xb) {
                placed.push(Placed { link, xa, xb });
            }
        });
        placed[short..].sort_unstable_by_key(|p| p.link.dep);
        placed
    }
}

/// Whether a run between x `a` and `b` meets the canvas; the arrowhead
/// reaches HEAD pixels behind its tip.
fn x_in(v: &Viewport, a: f64, b: f64) -> bool {
    a.max(b) + HEAD >= 0.0 && a.min(b) - HEAD <= v.width
}

/// x of the two ends of a link, CSS pixels: where it leaves the predecessor
/// and where it enters the successor.
pub(super) fn anchors(v: &Viewport, l: &Link, dates: &[TaskDates]) -> (f64, f64) {
    let (p0, p1) = bar_extent(v, &dates[l.pred as usize]);
    let (s0, s1) = bar_extent(v, &dates[l.succ as usize]);
    (
        if l.leaves_finish() { p1 } else { p0 },
        if l.enters_finish() { s1 } else { s0 },
    )
}

/// Whether a link is drawn: one of the five segments of its path crosses
/// the view, which is the canvas width by the visible `rows`. The path
/// leaves the predecessor at `xa` and turns after a stub, runs half a row
/// toward the successor, along the boundary between rows to a stub short of
/// the successor, down to its row, and enters it at `xb`. An end in view
/// qualifies, and so does a link whose ends are both outside but whose
/// vertical run or boundary run crosses it (plan D4).
pub(super) fn link_in_view(
    v: &Viewport,
    rows: &Range<usize>,
    link: &Link,
    xa: f64,
    xb: f64,
) -> bool {
    let (xo, xi) = (link.exit_x(xa), link.entry_x(xb));
    let row_in = |r: usize| rows.contains(&r);
    let x_in = |a: f64, b: f64| x_in(v, a, b);
    let (pred, succ) = (link.pred_row as usize, link.succ_row as usize);
    // The boundary run lies between the predecessor's row and the next one.
    let next = if succ > pred {
        pred + 1
    } else {
        pred.saturating_sub(1)
    };
    // Out of the predecessor: the stub and the half-row turn.
    (row_in(pred) && x_in(xa, xo))
        // Along the boundary to the successor's column.
        || ((row_in(pred) || row_in(next)) && x_in(xo, xi))
        // The vertical run down to the successor's row.
        || (link.lo() < rows.end && link.hi() >= rows.start && x_in(xi, xi))
        // Into the successor.
        || (row_in(succ) && x_in(xi, xb))
}

/// The visible part of a rectangle, CSS pixels: the canvas, a little wider
/// than itself so an edge is never drawn, and below the axis band.
pub(super) struct Clip {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

impl Builder {
    /// Push the part of the CSS-pixel rectangle that lies inside `clip`.
    fn clipped(&mut self, (x0, y0, x1, y1): (f64, f64, f64, f64), clip: &Clip) {
        let (x0, x1) = (x0.max(clip.x0), x1.min(clip.x1));
        let (y0, y1) = (y0.max(clip.y0), y1.min(clip.y1));
        if x0 < x1 && y0 < y1 {
            self.rect(x0, y0, x1, y1, self.arrow_color);
        }
    }

    /// A horizontal line `w` CSS pixels thick from xa to xb at y.
    pub(super) fn hseg(&mut self, xa: f64, xb: f64, y: f64, w: f64, clip: &Clip) {
        self.clipped((xa.min(xb), y - w / 2.0, xa.max(xb), y + w / 2.0), clip);
    }

    /// A vertical line `w` CSS pixels thick at x from ya to yb.
    pub(super) fn vseg(&mut self, x: f64, ya: f64, yb: f64, w: f64, clip: &Clip) {
        self.clipped((x - w / 2.0, ya.min(yb), x + w / 2.0, ya.max(yb)), clip);
    }

    /// An arrowhead with its tip at (x, y), pointing along `dir` (1 or -1): one triangle quad
    /// `HEAD` wide and `2 * HEAD - 1` tall, which the shader fills and antialiases. A quad that
    /// is not whole inside `clip` would draw a squeezed triangle, so it stays columns one pixel
    /// wide and 1, 3, 5, 7 pixels tall, each cut by the clip.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "HEAD is the constant 4.0, exact in i32"
    )]
    pub(super) fn head(&mut self, x: f64, y: f64, dir: f64, clip: &Clip) {
        let (x0, x1) = (x.min(x - dir * HEAD), x.max(x - dir * HEAD));
        let half = HEAD - 0.5;
        if x0 >= clip.x0 && x1 <= clip.x1 && y - half >= clip.y0 && y + half <= clip.y1 {
            let marker = if dir > 0.0 {
                TRIANGLE_RIGHT
            } else {
                TRIANGLE_LEFT
            };
            self.rounded(x0, y - half, x1, y + half, self.arrow_color, marker);
            return;
        }
        for i in 0..HEAD as i32 {
            let cx = x - dir * f64::from(i);
            let half = f64::from(i) + 0.5;
            self.clipped(
                (cx.min(cx - dir), y - half, cx.max(cx - dir), y + half),
                clip,
            );
        }
    }
}

/// Add the dependency arrows of the visible range to `scene`: for each link
/// that [`LinkIndex::visible`] returns, a path leaves the predecessor's bar,
/// turns at the boundary between rows, runs down or up to the successor's
/// row and ends in an arrowhead outside the successor's bar. The path is cut
/// at the canvas edges and the axis band.
/// The arrows go in the draw list just before the bars (`bar_start`), so a
/// bar is never covered by a line.
pub fn add_arrows(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    dates: &[TaskDates],
    links: &LinkIndex,
    theme: &Theme,
) {
    add_arrows_highlighted(scene, v, dpr, dates, links, None, None, theme);
}

/// Add visible dependency arrows, accenting highlighted links and dimming
/// all other links when a highlight is supplied. `selected` is a dependency id
/// and a width (`--gantt-selected-w`): that link is drawn that thick in
/// `sel_edge`, over a highlight, and after every other link so none paints over it.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
pub fn add_arrows_highlighted(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    dates: &[TaskDates],
    index: &LinkIndex,
    highlight: Option<&Highlight>,
    selected: Option<(u32, f64)>,
    theme: &Theme,
) {
    let mut b = Builder {
        dpr,
        arrow_color: theme.link,
        scene: Scene::default(),
    };
    let clip = Clip {
        x0: -2.0,
        x1: v.width + 2.0,
        y0: v.axis_height,
        y1: v.height,
    };
    // One pass over the visible links; the selected one is held back and drawn last.
    let mut draw = |p: &Placed, chosen: bool| {
        let width = selected.map_or(1.0, |(_, w)| if chosen { w } else { 1.0 });
        b.arrow_color = match highlight {
            _ if chosen => theme.sel_edge,
            Some(highlight) if highlight.has_edge(p.link.pred, p.link.succ) => theme.sel_edge,
            Some(_) => [
                theme.link[0],
                theme.link[1],
                theme.link[2],
                theme.link[3] * DIM,
            ],
            None => theme.link,
        };
        let before = b.scene.rects.len();
        let path = path(v, p);
        for seg in path.segs {
            match seg {
                Seg::H(xa, xb, y) => b.hseg(xa, xb, y, width, &clip),
                Seg::V(x, ya, yb) => b.vseg(x, ya, yb, width, &clip),
            }
        }
        b.head(path.head.0, path.head.1, path.head.2, &clip);
        b.scene.arrows += usize::from(b.scene.rects.len() > before);
    };
    let mut picked = None;
    for p in &index.visible(v, dates) {
        if selected.is_some_and(|(dep, _)| dep == p.link.dep) {
            picked = Some(*p);
        } else {
            draw(p, false);
        }
    }
    if let Some(p) = picked {
        draw(&p, true);
    }
    super::splice_under_bars(scene, b.scene.rects);
    scene.arrows += b.scene.arrows;
}
