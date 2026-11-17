//! Task names on or beside the bars (gs#45).
//!
//! [`add_bar_labels`] is a pass over the visible rows after [`crate::scene::build`],
//! like the arrows: one label per row at most. A name goes inside its bar when
//! it fits, to the right of the bar when the room up to the canvas edge allows,
//! and is left out otherwise. Rows under [`MIN_ROW_PX`] get none, and so does
//! every zoom coarser than the weeks zoom (gs#316). A name right of a task bar
//! starts past the bar's link terminal (`terminal_reach`, gs#440).
//!
//! The pass does not shape text. The caller passes `measure`, which returns the
//! width of a name in physical pixels (the renderer shapes it once and keeps
//! the buffer, see `crate::text::BarText`), so placement is plain arithmetic
//! and is tested natively with a fake measure.

use crate::axis::{Level, levels_for};
use crate::scene::bar_color::{done_ink, ink};
use crate::scene::{DIM, Highlight, Scene, bar_extent, bar_height, bar_top, span_extent};
use crate::summary::Summaries;
use crate::viewport::Viewport;
use crate::{HIDDEN_ROW, Theme};
use engine::TaskDates;

/// Rows shorter than this, CSS pixels, get no labels.
pub const MIN_ROW_PX: f64 = 16.0;
/// Room a name needs beyond its own width, CSS pixels: 4 on each side.
pub const PAD_PX: f64 = 8.0;
/// Bars with less visible width than this, CSS pixels, get no label: a name
/// beside a sliver is clutter, not information.
pub const MIN_BAR_PX: f64 = 8.0;
/// Largest label font size, CSS pixels.
pub const MAX_PX: f64 = 11.0;

/// Where a bar's name goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// On the bar.
    Inside,
    /// Just right of the bar.
    Right,
    /// Nowhere: there is no room.
    Hidden,
}

impl Placement {
    /// The word the debug export uses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Placement::Inside => "inside",
            Placement::Right => "right",
            Placement::Hidden => "none",
        }
    }
}

/// Decide where a name `text_w` wide goes, given the visible width of its bar
/// and the free `gap` to the right of the bar, less the `reach` of the link
/// terminal drawn there (all CSS pixels): right of the bar when the width plus
/// [`PAD_PX`] fits what is left of the gap (the name reads on the row
/// background, in the theme's text colour, however short the bar), else
/// inside when it fits a bar of at least [`MIN_BAR_PX`], else none.
#[must_use]
pub fn place(text_w: f64, bar_w: f64, gap: f64, reach: f64) -> Placement {
    if text_w + PAD_PX + reach <= gap {
        Placement::Right
    } else if bar_w >= MIN_BAR_PX && text_w + PAD_PX <= bar_w {
        Placement::Inside
    } else {
        Placement::Hidden
    }
}

/// Whether the zoom is the weeks zoom or finer: the coarsest zoom that names bars.
#[must_use]
pub fn names_at(v: &Viewport) -> bool {
    levels_for(v.px_per_minute).0 <= Level::Week
}

/// The label font size in CSS pixels for the rows of `v`: the bar's height
/// less two pixels, at most [`MAX_PX`]; none when the rows are under [`MIN_ROW_PX`].
#[must_use]
pub fn font_px(v: &Viewport) -> Option<f64> {
    (v.row_height >= MIN_ROW_PX).then(|| (bar_height(v) - 2.0).min(MAX_PX))
}

/// One placed bar label, physical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct BarLabel {
    /// The task the label names.
    pub task: u32,
    /// The name.
    pub text: String,
    /// Inside or right (a [`Placement::Hidden`] label is never stored).
    pub placement: Placement,
    /// Left edge of the text.
    pub x: f32,
    /// Top edge of the text.
    pub y: f32,
    /// Clip the text at this x.
    pub right: f32,
    /// Width of the name, CSS pixels (for tests: the inputs of [`place`]).
    pub text_w: f32,
    /// Room right of the bar for the name, CSS pixels (for tests): the free width up to the
    /// canvas edge, less the link terminal's reach for a task bar. The name goes right when
    /// `text_w + PAD_PX <= gap`.
    pub gap: f32,
    /// Font size.
    pub px: f32,
    /// Text colour, straight alpha.
    pub color: [f32; 4],
    /// For a name inside a bar whose progress quad ends at physical x `.0`: the colour of the part
    /// left of it, where `color` would not read on the quad.
    pub done: Option<(f32, [f32; 4])>,
}

/// The rows whose bars get labels.
#[derive(Debug, Clone, Copy)]
pub struct Rows<'a> {
    /// `order[r]` is the task on row r.
    pub order: &'a [u32],
    /// Dates by task.
    pub dates: &'a [TaskDates],
    /// Summary spans by task: a summary row draws a thinner bar over its span.
    pub summaries: &'a Summaries,
    /// A highlight dims the labels of the bars it dims.
    pub highlight: Option<&'a Highlight>,
    /// Colours used to draw the labels.
    pub theme: &'a Theme,
    /// How far the link terminal reaches past a task bar's finish end, CSS pixels:
    /// `terminal-gap + terminal-size`. A name beside a task bar starts past it, so
    /// the hovered bar's terminal is never under its name. Summary bars have no
    /// terminal and ignore it.
    pub terminal_reach: f64,
    /// Each proposed task's ghost ring, `(task, right edge in CSS pixels)`; a bar off the canvas has its name there.
    pub rings: &'a [(u32, f64)],
}

/// Place the names of the bars `build` drew into `scene.bar_labels`. `name`
/// gives the text of a task; `measure(task, text, px)` gives the width in
/// physical pixels of `text` at `px` physical pixels.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the font size and every name position are physical-pixel values: a name is placed only inside or beside a bar on the canvas, far under f32's 2^24 exact range"
)]
pub fn add_bar_labels<'a>(
    scene: &mut Scene,
    v: &Viewport,
    dpr: f64,
    rows: Rows<'_>,
    name: impl Fn(u32) -> &'a str,
    mut measure: impl FnMut(u32, &str, f32) -> f32,
) {
    let Rows {
        order,
        dates,
        summaries,
        highlight,
        theme,
        terminal_reach,
        rings,
    } = rows;
    scene.bar_labels.clear();
    let Some(css_px) = font_px(v).filter(|_| names_at(v)) else {
        return;
    };
    let px = (css_px * dpr) as f32;
    let line = f64::from(crate::text::line_height(px)) / dpr;
    let bh = bar_height(v);
    let width = v.width;
    for r in v.visible_rows() {
        let Some(&task) = order.get(r) else { break };
        if task == HIDDEN_ROW {
            continue;
        }
        let summary = summaries.get(task);
        let (x0, x1) = summary.map_or_else(
            || bar_extent(v, &dates[task as usize]),
            |(start, end)| span_extent(v, start, end),
        );
        // A bar off the canvas has no name, so its ghost ring, if any, takes it.
        let off = x1 < 0.0 || x0 > width;
        let ring = rings.iter().find(|r| r.0 == task && off);
        let (x0, x1) = ring.map_or((x0, x1), |r| (r.1, r.1));
        // A summary bar is drawn half as tall as a task bar (see `build`).
        let drawn = summary.map_or(bh, |_| (bh * 0.5).round().max(1.0));
        let y0 = bar_top(v, r);
        // Only whole bars, clear of the axis band and the bottom edge.
        if x1 < 0.0 || x0 > width || y0 < v.axis_height || y0 + bh > v.height {
            continue;
        }
        let text = name(task);
        if text.is_empty() {
            continue;
        }
        let text_w = f64::from(measure(task, text, px)) / dpr;
        let (left, right) = (x0.max(0.0), x1.min(width));
        let gap = (width - x1).max(0.0);
        let reach = if summary.is_some() {
            0.0
        } else {
            terminal_reach
        };
        let mut placement = place(text_w, right - left, gap, reach);
        // Text taller than its bar does not go inside. `place` already chose beside the bar
        // whenever the name fits there, so no room is left to try: the name is left out.
        if placement == Placement::Inside && drawn < css_px {
            placement = Placement::Hidden;
        }
        let (x, clip, color) = match placement {
            Placement::Inside => (left + PAD_PX / 2.0, right, ink(scene, task, theme)),
            Placement::Right => (x1 + reach + PAD_PX / 2.0, width, theme.text),
            Placement::Hidden => continue,
        };
        let color = match highlight {
            Some(h) if !h.has_task(task) => {
                let c = theme.text;
                [c[0], c[1], c[2], c[3] * DIM]
            }
            // A highlighted bar is solid `sel_edge`: its name takes `bar_text`, not `bar_ink`.
            Some(_) if placement == Placement::Inside => theme.bar_text,
            _ => color,
        };
        scene.bar_labels.push(BarLabel {
            task,
            text: text.to_string(),
            placement,
            x: (x * dpr).round() as f32,
            y: ((y0 + (bh - line) / 2.0) * dpr).round() as f32,
            right: (clip * dpr).round() as f32,
            text_w: text_w as f32,
            gap: (gap - reach).max(0.0) as f32,
            px,
            color,
            done: done_ink(scene, task, theme)
                .filter(|_| placement == Placement::Inside && highlight.is_none()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::build;
    use crate::viewport::ROW_HEIGHT;
    use engine::DAY;

    const THEME: Theme = Theme::light();
    /// The default link look: `terminal-gap` 3 plus `terminal-size` 8.
    const REACH: f64 = 11.0;

    #[test]
    fn placement_is_right_then_inside_then_none() {
        // Right whenever the gap takes the text plus 8 px, even beside a sliver.
        assert_eq!(place(50.0, 400.0, 58.0, 0.0), Placement::Right);
        assert_eq!(place(50.0, 2.0, 58.0, 0.0), Placement::Right);
        assert_eq!(place(50.0, 20.0, 57.9, 0.0), Placement::Hidden);
        // The terminal's reach comes off the gap.
        assert_eq!(place(50.0, 20.0, 69.0, 11.0), Placement::Right);
        assert_eq!(place(50.0, 20.0, 68.9, 11.0), Placement::Hidden);
        // No room to the right: inside needs the text plus 8 px of a bar of 8 px or more.
        assert_eq!(place(50.0, 58.0, 0.0, 0.0), Placement::Inside);
        assert_eq!(place(50.0, 57.9, 0.0, 0.0), Placement::Hidden);
        assert_eq!(place(0.0, 8.0, 0.0, 0.0), Placement::Inside);
        assert_eq!(place(0.0, 7.9, 0.0, 0.0), Placement::Hidden);
    }

    #[test]
    fn names_stop_past_the_weeks_zoom() {
        let mut v = Viewport::new(1200.0, 800.0, 1);
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        let per_day = |px: f64| px / DAY as f64;
        for (px, named) in [
            (20.0, true),
            (13.9, true),
            (6.9, true),
            (6.8, false),
            (1.0, false),
        ] {
            v.px_per_minute = per_day(px);
            assert_eq!(names_at(&v), named, "{px} px a day");
        }
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "n is a test's task count, a few dozen at most, far under i64::MAX"
    )]
    fn dates(n: usize) -> Vec<TaskDates> {
        (0..n as i64)
            .map(|i| TaskDates {
                es: i * DAY,
                ef: i * DAY + 8 * 60,
                ls: i * DAY + (i % 2) * DAY,
                lf: i * DAY + 8 * 60 + (i % 2) * DAY,
                float: (i % 2) * 480,
                finished: false,
            })
            .collect()
    }

    /// Six physical pixels a character at 11 px, scaled with the font.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a label's character count, far under f32's 2^24 exact range"
    )]
    fn width(_: u32, text: &str, px: f32) -> f32 {
        text.chars().count() as f32 * px * 6.0 / 11.0
    }

    fn names(task: u32) -> &'static str {
        ["Pour slab", "Set forms", "Place rebar"][task as usize % 3]
    }

    fn labelled(v: &Viewport, dpr: f64, d: &[TaskDates], h: Option<&Highlight>) -> Scene {
        labelled_with(v, dpr, d, &Summaries::new(d.len()), h)
    }

    fn labelled_with(
        v: &Viewport,
        dpr: f64,
        d: &[TaskDates],
        summaries: &Summaries,
        h: Option<&Highlight>,
    ) -> Scene {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "task ids are u32, so a task count fits u32"
        )]
        let order: Vec<u32> = (0..d.len() as u32).collect();
        let mut s = build(v, dpr, &order, d, summaries, None, None, &THEME);
        let rows = Rows {
            order: &order,
            dates: d,
            summaries,
            highlight: h,
            theme: &THEME,
            terminal_reach: REACH,
            rings: &[],
        };
        add_bar_labels(&mut s, v, dpr, rows, names, width);
        s
    }

    #[test]
    fn a_zoomed_in_view_names_its_visible_bars() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        // Eight hours over 1200 px: a bar is 1200 px wide and 8 h long.
        v.fit(0.0, 8.0 * 60.0);
        let s = labelled(&v, 1.0, &d, None);
        assert_eq!(s.bars, 1, "only the first task is in view");
        assert_eq!(s.bar_labels.len(), 1);
        let l = &s.bar_labels[0];
        assert_eq!(
            (l.task, l.placement, l.text.as_str()),
            (0, Placement::Inside, "Pour slab")
        );
        assert_eq!(l.color, THEME.bar_ink);
        assert!(l.x >= 4.0 && l.right > l.x);
        // The text line is centred on the bar, which is 14 px tall at 24 px rows.
        let bar = s.rects[s.bar_rects[0].1];
        let centre = |top: f32, h: f32| top + h / 2.0;
        let line = (l.px * 1.2).ceil();
        assert!((centre(l.y, line) - centre(bar.pos[1], bar.size[1])).abs() <= 1.0);
    }

    #[test]
    fn a_narrow_bar_gets_its_name_to_the_right_and_a_sliver_none() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        // 40 days over 1200 px is 30 px a day; a bar (8 hours of the 24-hour
        // day in these test dates) is 10 px wide.
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        v.fit(0.0, 40.0 * DAY as f64);
        let s = labelled(&v, 1.0, &d, None);
        // One bar per visible row, but the last is cut by the bottom edge and goes unnamed.
        let rows = v.visible_rows().len();
        assert_eq!(s.bars, rows);
        assert_eq!(s.bar_labels.len(), rows - 1);
        assert!(s.bar_labels.iter().all(|l| l.placement == Placement::Right));
        assert!(s.bar_labels.iter().all(|l| l.color == THEME.text));
        // Zoomed far out the bars are 2 px slivers: no names.
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        v.fit(0.0, 4000.0 * DAY as f64);
        let s = labelled(&v, 1.0, &d, None);
        assert_eq!(s.bars, v.visible_rows().len());
        assert_eq!(s.bar_labels, []);
        // A bar at the right edge has no room.
        let mut edge = dates(1);
        edge[0].es = 1000 * DAY;
        edge[0].ef = 1000 * DAY + 60;
        let mut v = Viewport::new(1200.0, 800.0, 1);
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        v.fit(0.0, 1000.0 * DAY as f64 + 60.0);
        let s = labelled(&v, 1.0, &edge, None);
        assert_eq!(s.bars, 1);
        assert_eq!(s.bar_labels, []);
    }

    #[test]
    fn short_rows_and_bars_under_the_axis_get_no_labels() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        v.fit(0.0, 8.0 * 60.0);
        v.row_height = 15.0;
        assert_eq!(labelled(&v, 1.0, &d, None).bar_labels, []);
        v.row_height = 16.0;
        assert_eq!(labelled(&v, 1.0, &d, None).bar_labels.len(), 1);
        // Scrolled half a row: the top bar is under the axis band, unlabelled.
        v.row_height = ROW_HEIGHT;
        v.scroll_by(ROW_HEIGHT / 2.0);
        let s = labelled(&v, 1.0, &d, None);
        assert!(s.bar_labels.iter().all(|l| l.task != 0));
    }

    #[test]
    fn one_label_a_row_and_the_scale_follows_the_pixel_ratio() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        v.fit(0.0, 4.0 * DAY as f64);
        let one = labelled(&v, 1.0, &d, None);
        let two = labelled(&v, 2.0, &d, None);
        assert_ne!(one.bar_labels, []);
        assert_eq!(one.bar_labels.len(), two.bar_labels.len());
        let rows = v.visible_rows().len();
        assert!(one.bar_labels.len() <= rows);
        for (a, b) in one.bar_labels.iter().zip(&two.bar_labels) {
            assert_eq!(a.task, b.task);
            assert_eq!(a.placement, b.placement);
            assert!((b.px - 2.0 * a.px).abs() < 1e-3);
            assert!((b.x - 2.0 * a.x).abs() <= 1.0);
        }
    }

    #[test]
    fn a_highlight_dims_the_labels_of_the_bars_it_dims() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        #[expect(
            clippy::cast_precision_loss,
            reason = "DAY is the constant 1440, exact in f64"
        )]
        v.fit(0.0, 4.0 * DAY as f64);
        let h = Highlight::new(40, [1], []);
        let s = labelled(&v, 1.0, &d, Some(&h));
        let alpha = |task| {
            s.bar_labels
                .iter()
                .find(|l| l.task == task)
                .map(|l| l.color[3])
        };
        assert_eq!(alpha(1), Some(1.0));
        assert_eq!(alpha(2), Some(DIM));
    }

    #[test]
    fn a_summary_bar_is_too_thin_for_text_inside_so_its_name_goes_beside_it() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        v.fit(0.0, 8.0 * 60.0);
        let mut sums = Summaries::new(40);
        sums.set(0, Some((0, 4 * 60)));
        let s = labelled_with(&v, 1.0, &d, &sums, None);
        assert_eq!(s.summary_bars, 1);
        assert_eq!(s.bar_labels.len(), 1);
        let l = &s.bar_labels[0];
        assert_eq!((l.task, l.placement), (0, Placement::Right));
        assert_eq!(l.color, THEME.text);
        // It starts after the summary's end (half the 1200 px view), plus 4.
        assert!((l.x - 604.0).abs() <= 1.0, "x {}", l.x);
        // No room beside it either: nothing.
        sums.set(0, Some((0, 8 * 60)));
        let s = labelled_with(&v, 1.0, &d, &sums, None);
        assert_eq!(s.bar_labels, []);
    }

    #[test]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "PAD_PX is the constant 8.0, exact in f32"
    )]
    fn a_task_bar_gap_leaves_out_the_terminal_reach_and_decides_the_placement() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        // The first bar ends 66 px short of the canvas edge: a 54 px name plus 8 fits that
        // (62), but not once the 11 px terminal is taken off (55).
        v.px_per_minute = 1134.0 / (8.0 * 60.0);
        v.t0 = 0.0;
        let s = labelled(&v, 1.0, &d, None);
        let l = &s.bar_labels[0];
        assert_eq!(l.task, 0);
        assert_eq!(l.placement, Placement::Inside);
        assert!((l.gap - 55.0).abs() < 0.5, "gap {}", l.gap);
        assert!(l.text_w + PAD_PX as f32 > l.gap);
        // A summary has no terminal: the same 66 px takes its name beside it.
        let mut sums = Summaries::new(40);
        sums.set(0, Some((0, 8 * 60)));
        let s = labelled_with(&v, 1.0, &d, &sums, None);
        let l = &s.bar_labels[0];
        assert_eq!(l.placement, Placement::Right);
        assert!((l.gap - 66.0).abs() < 0.5, "gap {}", l.gap);
    }

    #[test]
    fn critical_bars_still_have_white_text_inside() {
        let d = dates(40);
        let mut v = Viewport::new(1200.0, 800.0, 40);
        v.fit(0.0, 8.0 * 60.0);
        let s = labelled(&v, 1.0, &d, None);
        assert_eq!(s.rects[s.bar_rects[0].1].color, THEME.crit_soft);
        assert_eq!(s.bar_labels[0].color, THEME.bar_ink);
    }
}
