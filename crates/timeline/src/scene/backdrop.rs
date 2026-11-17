//! Shading of non-working days under the bars.

use super::{Builder, Rect};
use crate::Theme;
use crate::viewport::Viewport;
use engine::{Calendar, DAY};

/// The x ranges, CSS pixels, of the runs [`shade_non_working`] has pushed so far: the same list,
/// read back, so the axis band shades its minor tier without scanning the calendar again.
pub(super) fn shaded_runs(b: &Builder, v: &Viewport, theme: &Theme) -> Vec<(f64, f64)> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the axis band's height in physical pixels, tens, far under f32's 2^24 exact range"
    )]
    let (dpr, top) = (b.dpr, (v.axis_height * b.dpr) as f32);
    #[expect(
        clippy::float_cmp,
        reason = "a shading rect holds a bit copy of `theme.nonwork` and a y that `Builder::stroked` rounded to a whole pixel, as `top` is here, so both comparisons are exact"
    )]
    let is_shading = |r: &&Rect| r.color == theme.nonwork && r.pos[1] == top.round();
    b.scene
        .rects
        .iter()
        .filter(is_shading)
        .map(|r| {
            let x0 = f64::from(r.pos[0]) / dpr;
            (x0, x0 + f64::from(r.size[0]) / dpr)
        })
        .collect()
}

/// Mark the non-working days of `calendar` between instants `from` and `to`.
pub(super) fn shade_non_working(
    b: &mut Builder,
    v: &Viewport,
    calendar: &Calendar,
    from: i64,
    to: i64,
    theme: &Theme,
) {
    // Days before the origin hold no schedule; skip them.
    let first = from.max(0).div_euclid(DAY);
    let last = to.max(0).div_euclid(DAY);
    let mut run: Option<i64> = None;
    for day in first..=last + 1 {
        let off = day <= last && calendar.working_between(day * DAY, (day + 1) * DAY) == 0;
        match (off, run) {
            (true, None) => run = Some(day),
            #[expect(
                clippy::cast_precision_loss,
                reason = "start * DAY and day * DAY are instants of a day in view, far under 2^53 minutes (about 17 billion years)"
            )]
            (false, Some(start)) => {
                let x0 = v.x_of((start * DAY) as f64).max(-2.0);
                let x1 = v.x_of((day * DAY) as f64).min(v.width + 2.0);
                if x1 > x0 {
                    b.rect(x0, v.axis_height, x1, v.height, theme.nonwork);
                    b.scene.shaded += 1;
                }
                run = None;
            }
            _ => {}
        }
    }
}
