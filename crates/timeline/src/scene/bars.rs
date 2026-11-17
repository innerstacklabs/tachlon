//! The frame builder: row fills, grid, bars, summary brackets and the axis band.

use super::backdrop::shade_non_working;
use super::header::Header;
use super::{
    BRACKET_BAR, BRACKET_H, BRACKET_TICK, Backdrop, Builder, MIN_SHADE_DAY_PX, RowMarks, Scene,
    TAG_TEXT, bar_extent, bar_height, bar_top, span_extent,
};
use crate::summary::Summaries;
use crate::viewport::Viewport;
use crate::{HIDDEN_ROW, Theme};
use engine::{DAY, TaskDates};

/// The `Rect::radius` that marks a diamond: the shader reads -1 as "draw the quad as a diamond"
/// (below it, -2 and -3 are arrowhead triangles), so `Rect` keeps its layout.
pub(super) const DIAMOND: f64 = -1.0;

/// The width, CSS pixels, of the cap a start constraint puts on its bar's left edge, or a finish
/// constraint on its right edge.
const CONSTRAINT_CAP: f64 = 3.0;

/// Build the frame for viewport `v` at device pixel ratio `dpr`. `order[r]`
/// is the task shown on row r, or [`HIDDEN_ROW`] on a group header, which gets the summary
/// band and no bar; `dates` is indexed by task. `hover` is a row
/// to highlight. `dragged` is the task being dragged and whether its
/// predecessors hold it later than the pointer asks: that bar gets an
/// outline (amber when held). See [`build_marked`] for a selected row.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
#[must_use]
pub fn build(
    v: &Viewport,
    dpr: f64,
    order: &[u32],
    dates: &[TaskDates],
    summaries: &Summaries,
    hover: Option<usize>,
    dragged: Option<(u32, bool)>,
    theme: &Theme,
) -> Scene {
    let marks = RowMarks {
        hover,
        selected: None,
        range: None,
    };
    let backdrop = Backdrop::default();
    build_marked(
        v, dpr, order, dates, summaries, marks, dragged, backdrop, theme,
    )
}

/// [`build`] with the hovered and the selected row. A row's fill is, first
/// match wins: selected (`sel`), hovered (`hov`), a summary row (`band`), an
/// odd row (`stripe`). The task table applies the same order.
#[expect(
    clippy::too_many_arguments,
    reason = "the viewport, the scale, the task data and each style input arrive as separate arguments, and a parameter struct would change every caller and test"
)]
#[expect(clippy::too_many_lines, reason = "draw list is pushed in paint order")]
#[must_use]
pub fn build_marked(
    v: &Viewport,
    dpr: f64,
    order: &[u32],
    dates: &[TaskDates],
    summaries: &Summaries,
    marks: RowMarks,
    dragged: Option<(u32, bool)>,
    backdrop: Backdrop<'_>,
    theme: &Theme,
) -> Scene {
    let mut b = Builder {
        dpr,
        arrow_color: theme.link,
        scene: Scene::default(),
    };
    let (width, axis_h) = (v.width, v.axis_height);
    let rows = v.visible_rows();

    let in_range = |r: usize| {
        marks
            .range
            .is_some_and(|(first, last)| (first..=last).contains(&r))
    };
    // Row fills: selected, hovered, summary band, stripe.
    for r in rows.clone() {
        let y = v.row_top(r).max(axis_h);
        let y1 = v.row_top(r) + v.row_height;
        let summary_row = order
            .get(r)
            .is_some_and(|&t| t == HIDDEN_ROW || summaries.get(t).is_some());
        if Some(r) == marks.selected || in_range(r) {
            b.rect(0.0, y, width, y1, theme.sel);
        } else if Some(r) == marks.hover {
            b.rect(0.0, y, width, y1, theme.hov);
        } else if summary_row {
            b.rect(0.0, y, width, y1, theme.band);
        } else if r % 2 == 1 {
            b.rect(0.0, y, width, y1, theme.stripe);
        }
    }

    // Axis band and grid.
    let (t_from, t_to) = v.visible_time();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the visible time is minutes from the schedule origin, far inside i64"
    )]
    let (from, to) = (t_from.floor() as i64, t_to.ceil() as i64);
    let mut header = Header::new(v, backdrop.origin, from, to);

    // Non-working days: one column per run of consecutive days, over the row
    // fills and under the grid and the bars. Only drawn when a day is wider
    // than `MIN_SHADE_DAY_PX`, so the days scanned and the columns pushed are
    // bounded by the canvas width (at most `width / MIN_SHADE_DAY_PX` days and half
    // as many runs), never by the length of the schedule.
    #[expect(
        clippy::cast_precision_loss,
        reason = "DAY is the constant 1440, exact in f64"
    )]
    if let Some(calendar) = backdrop.calendar
        && v.px_per_minute * DAY as f64 > MIN_SHADE_DAY_PX
    {
        shade_non_working(&mut b, v, calendar, from, to, theme);
    }

    header.grid(&mut b, v, theme);

    // The data-date line runs from the minor tier to the bottom, over the grid
    // and under the bars; its pill sits in the minor tier.
    if let Some(date) = backdrop.data_date {
        header.data_date(&mut b, v, theme, date);
    }

    // Bars, visible rows only, culled to the canvas width.
    b.scene.bar_start = b.scene.rects.len();
    let bh = bar_height(v);
    for r in rows {
        let Some(&task) = order.get(r) else { break };
        if task == HIDDEN_ROW {
            continue;
        }
        let summary = summaries.get(task);
        let (x0, x1) = summary.map_or_else(
            || bar_extent(v, &dates[task as usize]),
            |(start, end)| span_extent(v, start, end),
        );
        if x1 < 0.0 || x0 > width {
            continue;
        }
        let y0 = bar_top(v, r);
        if summary.is_some() {
            // A thin bracket: a bar along the top with a tick down at each end.
            let sh = BRACKET_H.min(bh).max(1.0);
            let thick = BRACKET_BAR.min((sh / 2.0).floor()).max(1.0);
            let ys = y0 + ((bh - sh) / 2.0).round();
            if ys + sh <= axis_h {
                continue;
            }
            let bar_index = b.scene.rects.len();
            b.rect(
                x0.max(-2.0),
                ys.max(axis_h),
                x1.min(width + 2.0),
                ys + thick,
                theme.sum,
            );
            b.scene.bar_rects.push((task, bar_index));
            b.scene.summary_bars += 1;
            if x1 - x0 >= 2.0 * BRACKET_TICK {
                let ticks = [
                    (x0 >= -1.0).then_some((x0, x0 + BRACKET_TICK)),
                    (x1 <= width + 1.0).then_some((x1 - BRACKET_TICK, x1)),
                ];
                for (tx0, tx1) in ticks.into_iter().flatten() {
                    b.scene.bar_extras.push((task, b.scene.rects.len()));
                    b.rect(tx0, (ys + thick).max(axis_h), tx1, ys + sh, theme.sum);
                }
            }
            continue;
        }
        if y0 + bh <= axis_h {
            continue;
        }
        let row_dates = &dates[task as usize];
        let critical = row_dates.is_critical();
        // A bar is a pastel body with a saturated progress quad over its left part.
        let (soft, done, solid) = if critical {
            (theme.crit_soft, theme.crit_fill, theme.crit_fill)
        } else if let Some(c) = backdrop.bar_colors.and_then(|b| b.bar_color(task)) {
            b.scene.bar_colored.push((task, c));
            let soft = super::bar_color::soft(c, theme.bg);
            (soft, super::bar_color::progress(c, soft, theme), c)
        } else {
            (theme.bar_soft, theme.bar, theme.bar)
        };
        // A zero-duration task is a diamond centred on its date; its box is the square
        // `span_extent` gives, so a link arrow meets its left and right points.
        let milestone = row_dates.es == row_dates.ef;
        // A milestone has no progress to show, so its body is the solid colour.
        let body = if milestone { solid } else { soft };
        let radius = if milestone { DIAMOND } else { v.bar_radius };
        // A bar is cut at the canvas edge. A diamond keeps its whole box, a few pixels wide: cut
        // there it would draw lopsided (the GPU clips what lies past the canvas).
        let (cx0, cx1) = if milestone {
            (x0, x1)
        } else {
            (x0.max(-2.0), x1.min(width + 2.0))
        };
        if let Some((dt, held)) = dragged
            && dt == task
        {
            let pad = 2.0;
            let color = if held { theme.held } else { theme.text };
            let (ox0, oy0) = (cx0 - pad, (y0 - pad).max(axis_h));
            let r = if milestone {
                DIAMOND
            } else if radius > 0.0 {
                radius + pad
            } else {
                0.0
            };
            b.rounded(ox0, oy0, cx1 + pad, y0 + bh + pad, color, r);
        }
        // A critical bar is twice `crit_edge` taller than a normal one: a darker edge
        // (`--gantt-crit-edge-w`, heavier than a hairline so it reads without colour, tl#1215)
        // drawn first, then the body inside it. A bar with no room for both draws the body alone.
        // A diamond grows its edge on all four sides so it stays a diamond; its body keeps
        // the full box.
        let crit_w = v.crit_edge;
        let edge = critical && crit_w > 0.0 && x1 - x0 >= 2.0 * crit_w + 1.0;
        let inset = if edge && !milestone { crit_w } else { 0.0 };
        if edge {
            b.scene.bar_extras.push((task, b.scene.rects.len()));
            let (ey0, ey1) = (y0 - crit_w, y0 + bh + crit_w);
            let (ex0, ex1) = if milestone {
                (cx0 - crit_w, cx1 + crit_w)
            } else {
                (cx0, cx1)
            };
            let r = if milestone {
                DIAMOND
            } else if radius > 0.0 {
                radius + crit_w
            } else {
                0.0
            };
            b.rounded(ex0, ey0.max(axis_h), ex1, ey1, theme.crit_edge, r);
        }
        let bar_index = b.scene.rects.len();
        b.rounded(
            cx0 + inset,
            y0.max(axis_h),
            cx1 - inset,
            y0 + bh,
            body,
            radius,
        );
        b.scene.bar_rects.push((task, bar_index));
        // One more quad for a task with actuals (the engine's, not a percent): a finished task's
        // fills the bar, a started task's runs from its actual start, where the bar begins, to
        // the data date. Both are measured on the true extent (not the clipped one, so scrolling
        // does not move it).
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin, far under 2^53 (about 17 billion years), so f64 holds it exactly"
        )]
        let filled_to = backdrop
            .progress
            .and_then(|tasks| tasks.get(task as usize))
            .and_then(|t| {
                if t.actual_finish.is_some() {
                    Some(x1)
                } else if t.actual_start.is_some() {
                    backdrop.data_date.map(|date| v.x_of(date as f64).min(x1))
                } else {
                    None
                }
            });
        if let Some(fx) = filled_to
            && row_dates.ef > row_dates.es
            && x1 - x0 >= 3.0
        {
            let px1 = fx.min(cx1 - inset);
            if px1 > cx0 + inset {
                b.scene.bar_progress.push((task, b.scene.rects.len()));
                b.rounded(cx0 + inset, y0.max(axis_h), px1, y0 + bh, done, radius);
            }
        }
        // A task held by a real start constraint: a `text` cap on its bar's left edge, over the
        // body, taking its task's dimming. A finish constraint puts the same cap on the right edge.
        // Not on a bar whose edge is off the canvas (out of view) or one too narrow to keep its
        // body beside the cap.
        let (held_start, held_finish) = backdrop.start_constraints.map_or((false, false), |c| {
            (c.holds_start(task), c.holds_finish(task))
        });
        let room = x1 - x0 >= 2.0 * CONSTRAINT_CAP;
        if held_start && x0 >= -1.0 && room {
            b.scene.bar_extras.push((task, b.scene.rects.len()));
            b.rect(x0, y0.max(axis_h), x0 + CONSTRAINT_CAP, y0 + bh, theme.text);
        }
        if held_finish && x1 <= width + 1.0 && room {
            b.scene.bar_extras.push((task, b.scene.rects.len()));
            b.rect(x1 - CONSTRAINT_CAP, y0.max(axis_h), x1, y0 + bh, theme.text);
        }
        // A task that finishes after its deadline: a `crit` tick at the deadline, a little taller
        // than the bar so it shows over any bar colour, taking its task's dimming.
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin, far under 2^53, so f64 holds it exactly"
        )]
        let missed = backdrop
            .deadlines
            .and_then(|all| all.get(task as usize).copied().flatten())
            .filter(|&deadline| row_dates.ef > deadline)
            .map(|deadline| v.x_of(deadline as f64));
        if let Some(x) = missed.filter(|x| (-1.0..=width + 1.0).contains(x)) {
            b.scene.bar_extras.push((task, b.scene.rects.len()));
            b.rect(
                x - 1.0,
                (y0 - 2.0).max(axis_h),
                x + 1.0,
                y0 + bh + 2.0,
                theme.crit,
            );
        }
        b.scene.bars += 1;
        b.scene.critical_bars += usize::from(critical);
    }

    header.labels(&mut b, v, theme, backdrop.origin, TAG_TEXT);
    b.scene
}
