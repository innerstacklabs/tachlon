//! The comparison bars (gs#703): while a baseline is set, a thin bar under each task bar shows
//! where the task was planned, and a host's overlay (tl#2086) is a second one under it. One [`Rect`]
//! per visible leaf row and set of dates, drawn under the bars.

use super::outline::CAP;
use super::{Builder, Rect, Scene, bar_height, bar_top, span_extent, splice_under_bars};
use crate::summary::Summaries;
use crate::viewport::Viewport;
use engine::Instant;

/// Height of a comparison bar, CSS pixels.
pub const HEIGHT_PX: f64 = 3.0;

/// The gap between the bar and its comparison bar, CSS pixels.
pub const GAP_PX: f64 = 1.0;

/// The row background kept under a strip, CSS pixels: a later slot is lifted to keep it.
const ROW_FLOOR_PX: f64 = 2.0;

/// Add a comparison bar under each baselined visible leaf bar, under the bars. `order[row]` is the
/// task on `row`; `baseline_of(task)` gives its baseline start and finish, or `None` for a task
/// with no entry. Only the visible rows are walked, one lookup each. A summary row, a bar cut by
/// the axis band and a bar off the canvas get none. At most `CAP` rows are drawn, a guard a
/// canvas would need thousands of pixels of height to meet. The bar is `bar_height` tall in its
/// row, the comparison bar `HEIGHT_PX` under it after `GAP_PX`, from the baseline start to the
/// baseline finish; a task whose baseline equals its dates gets the same strip under its bar.
/// `strip` is the slot and the color: slot 0 is the strip under the bar, and each slot after it
/// sits `HEIGHT_PX + GAP_PX` lower, but is lifted until `ROW_FLOOR_PX` of its row is left under it
/// (never above slot 0): in a 24 px row every later slot shares slot 0's place. `other_held(task)`
/// says whether the task holds the other strip too: then in that row slot 0 takes the upper
/// `HEIGHT_PX - 1` of the place and the later slot the last pixel, so both show.
pub fn add_comparison_bars(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    summaries: &Summaries,
    order: &[u32],
    baseline_of: impl Fn(u32) -> Option<(Instant, Instant)>,
    (slot, color, other_held): (u32, [f32; 4], impl Fn(u32) -> bool),
) {
    let bh = bar_height(v);
    let mut b = Builder {
        dpr,
        arrow_color: color,
        scene: Scene::default(),
    };
    for row in v.visible_rows().take(CAP) {
        let Some(&task) = order.get(row) else {
            continue;
        };
        let top = bar_top(v, row);
        if top < v.axis_height || summaries.get(task).is_some() {
            continue;
        }
        let Some((start, finish)) = baseline_of(task) else {
            continue;
        };
        let (x0, x1) = span_extent(v, start, finish);
        if x1 < 0.0 || x0 > v.width {
            continue;
        }
        let first = top + bh + GAP_PX;
        let floor = v.row_top(row) + v.row_height - ROW_FLOOR_PX - HEIGHT_PX;
        let y = (first + f64::from(slot) * (HEIGHT_PX + GAP_PX))
            .min(floor)
            .max(first);
        let split = floor <= first && other_held(task);
        let (y, h) = match (split, slot) {
            (false, _) => (y, HEIGHT_PX),
            (true, 0) => (y, HEIGHT_PX - 1.0),
            (true, _) => (y + HEIGHT_PX - 1.0, 1.0),
        };
        b.rect(x0, y, x1, y + h, color);
    }
    let rects: Vec<Rect> = b.scene.rects;
    if !rects.is_empty() {
        splice_under_bars(scene, rects);
    }
}
