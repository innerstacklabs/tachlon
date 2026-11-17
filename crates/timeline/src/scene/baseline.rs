//! The comparison bars (gs#703): while a baseline is set, a thin bar under each task bar shows
//! where the task was planned. One [`Rect`] per baselined visible leaf row, drawn under the bars.

use super::outline::CAP;
use super::{Builder, Rect, Scene, bar_height, bar_top, span_extent, splice_under_bars};
use crate::Theme;
use crate::summary::Summaries;
use crate::viewport::Viewport;
use engine::Instant;

/// Height of a comparison bar, CSS pixels.
pub const HEIGHT_PX: f64 = 3.0;

/// The gap between the bar and its comparison bar, CSS pixels.
pub const GAP_PX: f64 = 1.0;

/// Add a comparison bar under each baselined visible leaf bar, under the bars. `order[row]` is the
/// task on `row`; `baseline_of(task)` gives its baseline start and finish, or `None` for a task
/// with no entry. Only the visible rows are walked, one lookup each. A summary row, a bar cut by
/// the axis band and a bar off the canvas get none. At most `CAP` rows are drawn, a guard a
/// canvas would need thousands of pixels of height to meet. The bar is `bar_height` tall in its
/// row, the comparison bar `HEIGHT_PX` under it after `GAP_PX`, from the baseline start to the
/// baseline finish; a task whose baseline equals its dates gets the same strip under its bar.
pub fn add_baseline_bars(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    summaries: &Summaries,
    order: &[u32],
    baseline_of: impl Fn(u32) -> Option<(Instant, Instant)>,
    theme: &Theme,
) {
    let bh = bar_height(v);
    let mut b = Builder {
        dpr,
        arrow_color: theme.link,
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
        let y = top + bh + GAP_PX;
        b.rect(x0, y, x1, y + HEIGHT_PX, theme.baseline);
    }
    let rects: Vec<Rect> = b.scene.rects;
    if !rects.is_empty() {
        splice_under_bars(scene, rects);
    }
}
