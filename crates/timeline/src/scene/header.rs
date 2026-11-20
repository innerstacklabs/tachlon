//! The axis band: two tiers of labels, the grid lines under them and the data-date pill.
//!
//! Every size here follows the band's height (`--gantt-axis-h`, [`Viewport::axis_height`]): a tier
//! is half the band, the text and the pill are shares of a tier. A host that sets another band
//! height gets the same proportions. Everything drawn is bounded by the canvas width, never by the
//! number of tasks: one line and one label per visible tick.

use super::Builder;
use super::backdrop::shaded_runs;
use crate::Theme;
use crate::axis::{self, Level, Origin};
use crate::names::Names;
use crate::viewport::Viewport;
use engine::Instant;

/// Average width of a glyph as a share of the font size, to size a label before it is shaped.
const GLYPH_EM: f64 = 0.55;

/// The sizes of the band, CSS pixels, all derived from the band's height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sizes {
    /// Height of one tier: half the band.
    pub(super) tier: f64,
    /// Gap between a tick and its label's box, and the pill's clearance: a sixth of a tier.
    pub(super) pad: f64,
    /// Font size of the major tier (the period names).
    pub(super) major_px: f64,
    /// Font size of the minor tier (the ticks).
    pub(super) minor_px: f64,
    /// Font size of the data-date pill.
    pub(super) tag_px: f64,
    /// Height of the data-date pill; its corners are half of it.
    pub(super) tag_h: f64,
    /// Width of the data-date pill.
    pub(super) tag_w: f64,
}

impl Sizes {
    pub(super) fn of(axis_height: f64) -> Sizes {
        let tier = axis_height / 2.0;
        Sizes {
            tier,
            pad: tier / 6.0,
            major_px: (tier * 0.5).round(),
            minor_px: (tier * 0.46).round(),
            tag_px: (tier * 0.42).round(),
            tag_h: (tier * 0.75).round(),
            tag_w: (tier * 2.8).round(),
        }
    }
}

/// The levels and ticks the band shows for one frame.
pub(super) struct Header {
    sizes: Sizes,
    minor: Level,
    major: Option<Level>,
    minor_ticks: Vec<i64>,
    major_ticks: Vec<i64>,
    /// The x range of the data-date pill, when there is one on the canvas.
    tag: Option<(f64, f64)>,
}

impl Header {
    /// The tiers for viewport `v` showing instants `from` to `to`.
    pub(super) fn new(v: &Viewport, origin: Origin, from: Instant, to: Instant) -> Header {
        let (minor, major) = axis::levels_for(v.px_per_minute);
        Header {
            sizes: Sizes::of(v.axis_height),
            minor,
            major,
            minor_ticks: minor.ticks(origin, from, to),
            major_ticks: major.map_or_else(Vec::new, |m| m.ticks(origin, from, to)),
            tag: None,
        }
    }

    /// The band, then a faint line at every minor tick (a day, at the weeks zoom and closer) and a
    /// stronger one at every major tick, all the way down.
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
    )]
    pub(super) fn grid(&self, b: &mut Builder, v: &Viewport, theme: &Theme) {
        let (width, height, axis_h) = (v.width, v.height, v.axis_height);
        b.rect(0.0, 0.0, width, axis_h, theme.axis);
        // Non-working runs continue up through the minor tier only, so the day letters sit on
        // the same shading as their columns and the week names stay on a plain band.
        for (x0, x1) in shaded_runs(b, v, theme) {
            b.rect(x0, self.sizes.tier, x1, axis_h, theme.nonwork);
        }
        for &t in &self.minor_ticks {
            let x = v.x_of(t as f64);
            if (0.0..width).contains(&x) {
                b.vline(x, self.sizes.tier, height, theme.grid);
            }
        }
        for &t in &self.major_ticks {
            let x = v.x_of(t as f64);
            if (0.0..width).contains(&x) {
                b.vline(x, 0.0, height, theme.grid_major);
            }
        }
        b.rect(0.0, axis_h - 1.0 / b.dpr, width, axis_h, theme.grid_major);
        if self.major.is_some() {
            b.rect(
                0.0,
                self.sizes.tier - 1.0 / b.dpr,
                width,
                self.sizes.tier,
                theme.grid,
            );
        }
    }

    /// The data-date line, from the minor tier to the bottom, and its pill in the minor tier. The
    /// pill hangs to the right of the line, or to the left near the right edge.
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
    )]
    pub(super) fn data_date(
        &mut self,
        b: &mut Builder,
        v: &Viewport,
        theme: &Theme,
        date: Instant,
    ) {
        let x = v.x_of(date as f64);
        if !(0.0..v.width).contains(&x) {
            return;
        }
        let sizes = self.sizes;
        let (tx0, tx1) = if x + sizes.tag_w <= v.width {
            (x, x + sizes.tag_w)
        } else {
            (x - sizes.tag_w + 1.0, x + 1.0)
        };
        // Dashed, so it reads apart from the critical red without colour (tl#1215): one rect per
        // dash, at most the line's height over dash and gap per frame, whatever the task count.
        let (dash, gap) = v.today_dash;
        let mut y = sizes.tier;
        while y < v.height {
            b.rect(x, y, x + 1.0, (y + dash).min(v.height), theme.today);
            y += dash + gap;
        }
        let top = sizes.tier + ((sizes.tier - sizes.tag_h) / 2.0).round();
        b.rounded(
            tx0,
            top,
            tx1,
            top + sizes.tag_h,
            theme.today,
            sizes.tag_h / 2.0,
        );
        b.scene.data_date_x = Some(x);
        self.tag = Some((tx0, tx1));
    }

    /// The labels: the major tier on top, the minor tier below it (alone in the lower tier when
    /// there is no major one). A minor label that would run under the pill moves clear of it, or is left out when it fits nowhere.
    pub(super) fn labels(
        &self,
        b: &mut Builder,
        v: &Viewport,
        theme: &Theme,
        origin: Origin,
        tag_text: &str,
    ) {
        let s = self.sizes;
        let names = Names::default();
        let text_y = |row: f64, px: f64| row * s.tier + (s.tier - px * 1.2) / 2.0;
        if let Some(major) = self.major {
            let spec = Spec {
                y: text_y(0.0, s.major_px),
                px: s.major_px,
                color: None,
                sticky: true,
                avoid: None,
                pad: s.pad,
                names: &names,
            };
            tier(b, v, origin, major, &self.major_ticks, &spec);
        }
        let spec = Spec {
            y: text_y(1.0, s.minor_px),
            px: s.minor_px,
            color: Some(theme.muted),
            sticky: false,
            avoid: self.tag,
            pad: s.pad,
            names: &names,
        };
        tier(b, v, origin, self.minor, &self.minor_ticks, &spec);
        if let Some((tx0, tx1)) = self.tag {
            let y = s.tier + ((s.tier - s.tag_h) / 2.0).round() + (s.tag_h - s.tag_px * 1.2) / 2.0;
            b.label(
                tag_text.to_string(),
                (tx0, tx1),
                y,
                s.tag_px,
                Some(theme.bar_text),
                true,
            );
        }
    }
}

/// One tier of labels.
struct Spec<'a> {
    /// Top of the text, CSS pixels.
    y: f64,
    /// Font size, CSS pixels.
    px: f64,
    /// Text colour; `None` is `Theme::text`.
    color: Option<[f32; 4]>,
    /// Period names stay on the canvas while their period is in view: a period is centred,
    /// except one cut off at the left edge, which sits at the left of the part that shows. A tick
    /// sits centred in its own column, or on the part of it in view.
    sticky: bool,
    /// An x range, CSS pixels, that labels of this tier keep clear of.
    avoid: Option<(f64, f64)>,
    /// Gap between a tick and its label's box, CSS pixels ([`Sizes::pad`]).
    pad: f64,
    /// The month and weekday names the labels use.
    names: &'a Names,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a tick is an instant, far under 2^53 minutes (about 17 billion years), and a label has a few dozen characters, so f64 holds both exactly"
)]
fn tier(
    b: &mut Builder,
    v: &Viewport,
    origin: Origin,
    level: Level,
    ticks: &[i64],
    spec: &Spec<'_>,
) {
    let tick_px = level.typical_minutes() * v.px_per_minute;
    for &t in ticks {
        let x = v.x_of(t as f64);
        let next = v.x_of(level.next(origin, t) as f64);
        // A column cut by either edge is named on the part in view, so a schedule a couple of
        // periods wide does not lose the label of the column at each end.
        let (lo, hi) = (x.max(0.0), next.min(v.width));
        let text = if spec.sticky {
            level.long_label(origin, t, spec.names)
        } else {
            level.minor_label(origin, t, tick_px, spec.names)
        };
        // The text is about half an em per character; it only has to clear its neighbours.
        let text_w = text.chars().count() as f64 * spec.px * GLYPH_EM;
        let centre = f64::midpoint(lo, hi);
        // A label the pill would cover moves to the widest clear part of its column; it is
        // dropped only when no part holds the text and its padding.
        let (mut lo, mut hi) = (lo, hi);
        if let Some((a0, a1)) = spec.avoid
            && centre - text_w / 2.0 < a1 + spec.pad
            && centre + text_w / 2.0 > a0 - spec.pad
        {
            let left = (lo, (a0 - spec.pad).min(hi));
            let right = ((a1 + spec.pad).max(lo), hi);
            let widest = if left.1 - left.0 >= right.1 - right.0 {
                left
            } else {
                right
            };
            if text_w + spec.pad > widest.1 - widest.0 {
                continue;
            }
            (lo, hi) = widest;
        }
        if spec.sticky && text_w + 2.0 * spec.pad > hi - lo {
            // A period cut by the right edge whose part in view cannot hold its name leaves it out;
            // one cut by the left edge keeps its name at the edge, clipped.
            if x >= 0.0 && next > v.width {
                continue;
            }
            if hi - lo >= 2.0 * spec.px {
                b.label(
                    text,
                    (lo + spec.pad, hi - spec.pad / 2.0),
                    spec.y,
                    spec.px,
                    spec.color,
                    false,
                );
            }
            continue;
        }
        if spec.sticky && x < 0.0 {
            // A period cut off at the left edge sticks to the visible start instead of drifting
            // to the middle of what shows.
            b.label(
                text,
                (lo + spec.pad, hi - spec.pad / 2.0),
                spec.y,
                spec.px,
                spec.color,
                false,
            );
            continue;
        }
        if !spec.sticky && text_w + spec.pad > hi - lo {
            continue;
        }
        b.label(text, (lo, hi), spec.y, spec.px, spec.color, true);
    }
}
