//! The five-segment path of a link and the arrowhead at its end: one shape,
//! drawn by `add_arrows_highlighted` and measured by `LinkIndex::hit`.

use super::links::{Link, Placed};
use crate::viewport::Viewport;
use engine::LinkType;

/// How far a link leaves its first bar before it turns, CSS pixels.
pub const STUB: f64 = 6.0;
/// Length of an arrowhead, CSS pixels.
pub const HEAD: f64 = 4.0;

/// One straight run of a path, CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Seg {
    /// `H(xa, xb, y)`: along a row.
    H(f64, f64, f64),
    /// `V(x, ya, yb)`: across rows.
    V(f64, f64, f64),
}

/// A link's path: five runs and the arrowhead's tip and direction (1 or -1).
pub(super) struct Path {
    pub segs: [Seg; 5],
    pub head: (f64, f64, f64),
}

/// The direction a path travels into a bar's `finish` or start: left into a
/// finish, right into a start (-1 or 1).
fn in_dir(finish: bool) -> f64 {
    if finish { -1.0 } else { 1.0 }
}

/// x where a path turns before entering a bar at `xb`, which is the bar's
/// `finish` or its start.
pub(super) fn entry_x(finish: bool, xb: f64) -> f64 {
    xb - in_dir(finish) * STUB
}

impl Link {
    /// The path leaves the predecessor's finish (FS, FF) or start (SS, SF).
    pub(super) fn leaves_finish(&self) -> bool {
        matches!(self.kind, LinkType::FS | LinkType::FF)
    }

    /// The path enters the successor's finish (FF, SF) or start (FS, SS).
    pub(super) fn enters_finish(&self) -> bool {
        matches!(self.kind, LinkType::FF | LinkType::SF)
    }

    /// x where the path turns after leaving the predecessor at `xa`: a stub
    /// beyond that bar's end.
    pub(super) fn exit_x(&self, xa: f64) -> f64 {
        xa + if self.leaves_finish() { STUB } else { -STUB }
    }

    /// The direction the path travels into the successor: right into a start,
    /// left into a finish (1 or -1).
    pub(super) fn in_dir(&self) -> f64 {
        in_dir(self.enters_finish())
    }

    /// x where the path turns before entering the successor at `xb`: a stub
    /// outside that bar, so the arrowhead is never inside it.
    pub(super) fn entry_x(&self, xb: f64) -> f64 {
        entry_x(self.enters_finish(), xb)
    }

    /// The vertical direction of the path: 1 down the rows, -1 up.
    pub(super) fn down(&self) -> f64 {
        if self.succ_row > self.pred_row {
            1.0
        } else {
            -1.0
        }
    }
}

/// The path of a placed link: out of the predecessor and a stub on, half a
/// row toward the successor, along the boundary between rows, down to the
/// successor's row, and in. The run across is made on the boundary between
/// the predecessor's row and the next toward the successor, clear of every bar.
pub(super) fn path(v: &Viewport, p: &Placed) -> Path {
    let y_of = |row: u32| v.row_top(row as usize) + v.row_height / 2.0;
    let (ya, yb) = (y_of(p.link.pred_row), y_of(p.link.succ_row));
    let ym = ya + p.link.down() * v.row_height / 2.0;
    let (xo, xi) = (p.link.exit_x(p.xa), p.link.entry_x(p.xb));
    Path {
        segs: [
            Seg::H(p.xa, xo, ya),
            Seg::V(xo, ya, ym),
            Seg::H(xo, xi, ym),
            Seg::V(xi, ym, yb),
            Seg::H(xi, p.xb, yb),
        ],
        head: (p.xb, yb, p.link.in_dir()),
    }
}
