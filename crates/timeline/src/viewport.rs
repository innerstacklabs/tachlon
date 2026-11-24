//! Viewport state: which slice of time and which rows the canvas shows.
//!
//! All lengths here are CSS pixels; the renderer multiplies by the device
//! pixel ratio. Time is in minutes from the project origin as `f64`, and an
//! instant maps to x as `(t - t0) * px_per_minute` (plan D5), never through
//! epoch milliseconds.

use core::ops::Range;
use engine::DAY;

/// Narrowest span the canvas may show: two hours across its width.
pub const MIN_SPAN: f64 = 120.0;
/// Widest span the canvas may show: five years across its width.
#[expect(
    clippy::cast_precision_loss,
    reason = "DAY is the constant 1440, exact in f64"
)]
pub const MAX_SPAN: f64 = 5.0 * 365.25 * DAY as f64;

/// Default row height: [`Density::Normal`].
pub const ROW_HEIGHT: f64 = Density::Normal.row_height();
/// Height of the time-axis band at the top of the canvas: two tiers of 24 px (`--gantt-axis-h`).
pub const AXIS_HEIGHT: f64 = 48.0;
/// Default bar height as a share of the row (`--gantt-bar-h`).
pub const BAR_RATIO: f64 = 0.50;
/// Default corner radius of a bar, CSS pixels (`--gantt-bar-r`).
pub const BAR_RADIUS: f64 = 2.0;
/// Default dash and gap of the data-date line, CSS pixels (`--gantt-today-dash`, `--gantt-today-gap`).
pub const TODAY_DASH: (f64, f64) = (6.0, 4.0);
/// Default width of a critical bar's edge, CSS pixels (`--gantt-crit-edge-w`).
pub const CRIT_EDGE: f64 = 2.0;

/// How tall the rows are, which a host picks by name: compact 20 px, normal 24 px, roomy 40 px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    /// 20 px rows.
    Compact,
    /// 24 px rows, the default.
    Normal,
    /// 40 px rows.
    Roomy,
}

impl Density {
    /// `"compact"`, `"normal"` or `"roomy"`, in lower case only; anything else is `None`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "compact" => Some(Self::Compact),
            "normal" => Some(Self::Normal),
            "roomy" => Some(Self::Roomy),
            _ => None,
        }
    }

    /// The height of one row in CSS pixels.
    #[must_use]
    pub const fn row_height(self) -> f64 {
        match self {
            Self::Compact => 20.0,
            Self::Normal => 24.0,
            Self::Roomy => 40.0,
        }
    }
}

/// The sizes a session keeps when it opens another file: the row and axis heights and the bar look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Height of one row.
    pub row_height: f64,
    /// Height of the time-axis band.
    pub axis_height: f64,
    /// Bar height as a share of the row.
    pub bar_ratio: f64,
    /// Corner radius of a bar, CSS pixels.
    pub bar_radius: f64,
    /// Dash and gap of the data-date line, CSS pixels.
    pub today_dash: (f64, f64),
    /// Width of a critical bar's edge, CSS pixels.
    pub crit_edge: f64,
}

/// What the canvas shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    /// Instant at the left edge of the canvas.
    pub t0: f64,
    /// Horizontal scale.
    pub px_per_minute: f64,
    /// Vertical scroll of the row area: 0 puts row 0 just under the axis band.
    pub scroll_y: f64,
    /// Height of one row.
    pub row_height: f64,
    /// Height of the time-axis band.
    pub axis_height: f64,
    /// Bar height as a share of the row (`--gantt-bar-h`).
    pub bar_ratio: f64,
    /// Corner radius of a bar, CSS pixels (`--gantt-bar-r`).
    pub bar_radius: f64,
    /// Dash and gap of the data-date line, CSS pixels (`--gantt-today-dash`, `--gantt-today-gap`).
    pub today_dash: (f64, f64),
    /// Width of a critical bar's edge, CSS pixels (`--gantt-crit-edge-w`).
    pub crit_edge: f64,
    /// Canvas width.
    pub width: f64,
    /// Canvas height.
    pub height: f64,
    /// Number of rows in the schedule.
    pub rows: usize,
    /// Height the table's sideways scrollbar takes from the table body: the last row needs this much more scroll to clear it.
    pub scroll_slack: f64,
}

impl Viewport {
    /// A viewport of `width` x `height` over `rows` rows, showing the widest
    /// span from the origin.
    #[must_use]
    pub fn new(width: f64, height: f64, rows: usize) -> Viewport {
        let width = width.max(1.0);
        Viewport {
            t0: 0.0,
            px_per_minute: width / MAX_SPAN,
            scroll_y: 0.0,
            row_height: ROW_HEIGHT,
            axis_height: AXIS_HEIGHT,
            bar_ratio: BAR_RATIO,
            bar_radius: BAR_RADIUS,
            today_dash: TODAY_DASH,
            crit_edge: CRIT_EDGE,
            width,
            height: height.max(1.0),
            rows,
            scroll_slack: 0.0,
        }
    }

    /// The sizes that outlive a file: see [`Metrics`].
    #[must_use]
    pub fn metrics(&self) -> Metrics {
        Metrics {
            row_height: self.row_height,
            axis_height: self.axis_height,
            bar_ratio: self.bar_ratio,
            bar_radius: self.bar_radius,
            today_dash: self.today_dash,
            crit_edge: self.crit_edge,
        }
    }

    /// Take `m` as this viewport's row and axis heights and bar look.
    pub fn set_metrics(&mut self, m: Metrics) {
        self.row_height = m.row_height;
        self.axis_height = m.axis_height;
        self.bar_ratio = m.bar_ratio;
        self.bar_radius = m.bar_radius;
        self.today_dash = m.today_dash;
        self.crit_edge = m.crit_edge;
    }

    /// Allowed range of `px_per_minute` for the current width.
    #[must_use]
    pub fn scale_range(&self) -> (f64, f64) {
        (self.width / MAX_SPAN, self.width / MIN_SPAN)
    }

    /// x of instant `t`.
    #[must_use]
    pub fn x_of(&self, t: f64) -> f64 {
        (t - self.t0) * self.px_per_minute
    }

    /// Instant at x.
    #[must_use]
    pub fn t_of(&self, x: f64) -> f64 {
        self.t0 + x / self.px_per_minute
    }

    /// Instants at the left and right edges.
    #[must_use]
    pub fn visible_time(&self) -> (f64, f64) {
        (self.t0, self.t_of(self.width))
    }

    /// Height of the row area below the axis band.
    #[must_use]
    pub fn body_height(&self) -> f64 {
        (self.height - self.axis_height).max(0.0)
    }

    /// Largest `scroll_y`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "rows is a row count, far under 2^53"
    )]
    #[must_use]
    pub fn max_scroll(&self) -> f64 {
        (self.rows as f64 * self.row_height - self.body_height() + self.scroll_slack).max(0.0)
    }

    /// Rows that intersect the row area.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "each quotient is a row number from a non-negative offset over a positive row height, far under usize::MAX"
    )]
    #[must_use]
    pub fn visible_rows(&self) -> Range<usize> {
        let first = (self.scroll_y / self.row_height).floor().max(0.0) as usize;
        let last = ((self.scroll_y + self.body_height()) / self.row_height).ceil() as usize;
        first.min(self.rows)..last.min(self.rows)
    }

    /// Top y of row `row` (may be off the canvas).
    #[expect(clippy::cast_precision_loss, reason = "a row index, far under 2^53")]
    #[must_use]
    pub fn row_top(&self, row: usize) -> f64 {
        self.axis_height + row as f64 * self.row_height - self.scroll_y
    }

    /// Row under y, if y is in the row area and on a row.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "r is checked non-negative and below the row count in the same expression"
    )]
    #[must_use]
    pub fn row_at(&self, y: f64) -> Option<usize> {
        if y < self.axis_height || y >= self.height {
            return None;
        }
        let r = ((y - self.axis_height + self.scroll_y) / self.row_height).floor();
        (r >= 0.0 && (r as usize) < self.rows).then_some(r as usize)
    }

    /// Resize the canvas, keeping the instant at the left edge and the scale
    /// (re-clamped: the span limits depend on the width).
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        let (lo, hi) = self.scale_range();
        self.px_per_minute = self.px_per_minute.clamp(lo, hi);
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll());
    }

    /// Multiply the scale by `factor`, clamped, keeping the instant under x
    /// at x.
    pub fn zoom_at(&mut self, x: f64, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let t = self.t_of(x);
        let (lo, hi) = self.scale_range();
        self.px_per_minute = (self.px_per_minute * factor).clamp(lo, hi);
        self.t0 = t - x / self.px_per_minute;
    }

    /// Move the content by (dx, dy) pixels: content follows the pointer.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.t0 -= dx / self.px_per_minute;
        self.scroll_by(-dy);
    }

    /// Scroll so row `row` is the top row (as far as the scroll range allows).
    #[expect(clippy::cast_precision_loss, reason = "a row index, far under 2^53")]
    pub fn scroll_to_row(&mut self, row: usize) {
        self.scroll_y = (row as f64 * self.row_height).clamp(0.0, self.max_scroll());
    }

    /// Scroll the rows by dy pixels (positive = down the schedule).
    pub fn scroll_by(&mut self, dy: f64) {
        self.scroll_y = (self.scroll_y + dy).clamp(0.0, self.max_scroll());
    }

    /// Show `[start, end]` with a small margin on both sides. A span wider
    /// than [`MAX_SPAN`] shows its first five years.
    pub fn fit(&mut self, start: f64, end: f64) {
        let span = (end - start).max(MIN_SPAN) * 1.04;
        let (lo, hi) = self.scale_range();
        self.px_per_minute = (self.width / span).clamp(lo, hi);
        self.t0 = start - 0.02 * self.width / self.px_per_minute;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> Viewport {
        Viewport::new(1200.0, 800.0, 50_000)
    }

    #[test]
    fn scroll_slack_lets_the_last_row_clear_the_sideways_scrollbar() {
        let mut v = vp();
        let flat = v.max_scroll();
        v.scroll_slack = 15.0;
        assert_eq!(v.max_scroll(), flat + 15.0);
        v.scroll_by(1e12);
        assert_eq!(v.scroll_y, flat + 15.0);
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "DAY is the constant 1440, exact in f64"
    )]
    fn zoom_keeps_the_instant_under_the_cursor() {
        let mut v = vp();
        v.fit(0.0, 400.0 * DAY as f64);
        for (x, f) in [
            (0.0, 1.5),
            (317.0, 1.1),
            (1199.0, 0.7),
            (600.0, 1e6),
            (5.0, 1e-9),
        ] {
            let t = v.t_of(x);
            v.zoom_at(x, f);
            assert!((v.x_of(t) - x).abs() < 1e-6, "x {x} factor {f}");
        }
    }

    #[test]
    fn zoom_clamps_to_two_hours_and_five_years() {
        let mut v = vp();
        v.zoom_at(100.0, 1e12);
        let (a, b) = v.visible_time();
        assert!((b - a - MIN_SPAN).abs() < 1e-6);
        v.zoom_at(100.0, 1e-12);
        let (a, b) = v.visible_time();
        assert!((b - a - MAX_SPAN).abs() < 1e-3);
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "DAY is the constant 1440, exact in f64; the body height over the row height is a small non-negative row count"
    )]
    fn pan_moves_content_with_the_pointer() {
        let mut v = vp();
        v.fit(0.0, 30.0 * DAY as f64);
        let t = v.t_of(500.0);
        v.pan(40.0, 0.0);
        assert!((v.x_of(t) - 540.0).abs() < 1e-9);
        v.pan(0.0, -10.0 * ROW_HEIGHT);
        assert_eq!(v.scroll_y, 10.0 * ROW_HEIGHT);
        let first = v.visible_rows().start;
        assert_eq!(first, 10);
        assert_eq!(
            v.visible_rows().len(),
            (v.body_height() / ROW_HEIGHT).ceil() as usize
        );
        v.pan(0.0, 1e9);
        assert_eq!(v.scroll_y, 0.0);
        v.pan(0.0, -1e12);
        assert_eq!(v.scroll_y, v.max_scroll());
        assert_eq!(v.visible_rows().end, 50_000);
    }

    #[test]
    fn rows_under_y() {
        let mut v = vp();
        assert_eq!(v.row_at(10.0), None);
        assert_eq!(v.row_at(AXIS_HEIGHT), Some(0));
        assert_eq!(v.row_at(AXIS_HEIGHT + ROW_HEIGHT * 3.5), Some(3));
        v.scroll_by(ROW_HEIGHT * 100.0);
        assert_eq!(v.row_at(AXIS_HEIGHT + 1.0), Some(100));
        assert_eq!(v.row_top(100), AXIS_HEIGHT);
        let small = Viewport::new(100.0, 800.0, 2);
        assert_eq!(small.row_at(AXIS_HEIGHT + ROW_HEIGHT * 2.5), None);
        assert_eq!(small.visible_rows(), 0..2);
    }

    #[test]
    fn resize_reclamps_the_scale() {
        let mut v = vp();
        v.zoom_at(0.0, 1e12);
        v.set_size(300.0, 800.0);
        assert_eq!(v.px_per_minute, 300.0 / MIN_SPAN);
    }

    #[test]
    fn a_density_is_parsed_by_name_and_has_one_height() {
        let all = [Density::Compact, Density::Normal, Density::Roomy];
        assert_eq!(
            ["compact", "normal", "roomy"].map(Density::parse),
            all.map(Some)
        );
        assert_eq!(["", "tall", "Roomy"].map(Density::parse), [None; 3]);
        assert_eq!(Density::Normal.row_height(), ROW_HEIGHT);
        assert!(
            all.windows(2)
                .all(|w| w[0].row_height() < w[1].row_height())
        );
    }
}
