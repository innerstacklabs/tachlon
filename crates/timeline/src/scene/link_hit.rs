//! Finding the link under a point.

use super::link_path::{HEAD, Seg, path};
use super::links::LinkIndex;
use super::{bar_height, bar_top};
use crate::{Timeline, viewport::Viewport};
use engine::TaskDates;

/// How far from a link's path a point still hits it, CSS pixels.
pub const HIT_SLOP: f64 = 4.0;

/// Distance from (x, y) to the box `[x0, x1] x [y0, y1]`.
fn dist(x: f64, y: f64, (x0, y0, x1, y1): (f64, f64, f64, f64)) -> f64 {
    let dx = (x0 - x).max(x - x1).max(0.0);
    let dy = (y0 - y).max(y - y1).max(0.0);
    dx.hypot(dy)
}

impl LinkIndex {
    /// The id of the link whose path or arrowhead is nearest (x, y), within
    /// [`HIT_SLOP`], among the links the view shows; the lowest id on a tie. The
    /// arrowhead is measured as its own box, so a point beside the head that
    /// no run reaches still hits.
    /// Only visible links are measured, so the cost follows the view, not the
    /// schedule. Bars are not considered here: see [`Timeline::hit_link`].
    pub fn hit(&self, v: &Viewport, dates: &[TaskDates], x: f64, y: f64) -> Option<u32> {
        if y < v.axis_height || y >= v.height {
            return None;
        }
        let mut best: Option<(f64, u32)> = None;
        for p in self.visible(v, dates) {
            let path = path(v, &p);
            let (tx, ty, dir) = path.head;
            let head = (
                tx.min(tx - dir * HEAD),
                ty - HEAD,
                tx.max(tx - dir * HEAD),
                ty + HEAD,
            );
            let near = path
                .segs
                .iter()
                .map(|seg| match *seg {
                    Seg::H(x0, x1, sy) => (x0.min(x1), sy, x0.max(x1), sy),
                    Seg::V(sx, y0, y1) => (sx, y0.min(y1), sx, y0.max(y1)),
                })
                .chain([head])
                .map(|rect| dist(x, y, rect))
                .fold(f64::INFINITY, f64::min);
            #[expect(
                clippy::float_cmp,
                reason = "a tie is two links at the same computed distance, where the lowest id wins; a tolerance would change which link a press picks"
            )]
            let better = best
                .is_none_or(|(nearest, id)| near < nearest || (near == nearest && p.link.dep < id));
            if near <= HIT_SLOP && better {
                best = Some((near, p.link.dep));
            }
        }
        best.map(|(_, id)| id)
    }
}

impl Timeline {
    /// What a press at (x, y) lands on among the links: the link's dependency
    /// id, unless a bar is under the point, which wins. A bar owns only the
    /// pixels it is drawn on, not the whole row, so a link along a row boundary
    /// stays hittable beside a bar's edge.
    #[must_use]
    pub fn hit_link(&self, links: &LinkIndex, dates: &[TaskDates], x: f64, y: f64) -> Option<u32> {
        if self.hit_test(dates, x, y).is_some() && self.on_bar_body(y) {
            return None;
        }
        links.hit(&self.viewport, dates, x, y)
    }

    /// Whether `y` is within the vertical extent of the bar drawn on its row.
    fn on_bar_body(&self, y: f64) -> bool {
        self.viewport.row_at(y).is_some_and(|row| {
            let top = bar_top(&self.viewport, row);
            top <= y && y < top + bar_height(&self.viewport)
        })
    }
}
