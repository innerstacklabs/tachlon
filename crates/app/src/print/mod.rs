//! The print plan and the page scene (gs#703): which rows go on paper, how many pages they take,
//! and the draw list of one page. Print runs on `beforeprint`, not per frame.

mod legend;
mod months;
mod page;
mod widths;

use crate::{Session, lookahead::active_in};
use engine::Instant;
use timeline::{
    Scene, Theme, Viewport,
    scene::{RowMarks, add_arrows_marked, build_marked},
    viewport::AXIS_HEIGHT,
};

/// Most pages a plan holds; the rows past them are counted in [`PrintPlan::left_out`].
pub const MAX_PAGES: usize = 100;

// One printed page, landscape. Nothing here reads the window.
const MARGIN: f64 = 36.0;
const HEADER_H: f64 = 44.0;
const FOOTER_H: f64 = 20.0;

/// The sheet a page is printed on: US Letter (1056 by 816, the default) or A4 (1123 by 794), both in CSS px at 96 to the inch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PageSize {
    #[default]
    Letter,
    A4,
}

impl PageSize {
    /// `"letter"` or `"a4"`, in any case.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "letter" => Some(Self::Letter),
            "a4" => Some(Self::A4),
            _ => None,
        }
    }

    /// Width and height of the sheet, landscape.
    #[must_use]
    pub const fn dims(self) -> (f64, f64) {
        match self {
            Self::Letter => (1056.0, 816.0),
            Self::A4 => (1123.0, 794.0),
        }
    }

    /// The whole rows of `row_height` px that fit under the axis in the body of the shorter sheet,
    /// so Letter and A4 hold the same count (25 at 24 px).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "body is the page height minus constant margins; a float-to-usize `as` saturates, so a negative or NaN quotient becomes 0 and it never wraps"
    )]
    fn rows_per_page(row_height: f64) -> usize {
        let page_h = Self::Letter.dims().1.min(Self::A4.dims().1);
        let body = page_h - 2.0 * MARGIN - HEADER_H - FOOTER_H - AXIS_HEIGHT;
        (body / row_height) as usize
    }
}

/// Rows of the page and the scale that fits the printed rows' span into the panel width.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintPlan {
    /// The tasks on the printed rows, in row order, at most `MAX_PAGES * rows_per_page` of them.
    pub rows: Vec<u32>,
    /// The sheet the pages are printed on.
    pub size: PageSize,
    /// Rows on a full page: set by the sheet, whatever the window.
    pub rows_per_page: usize,
    /// Pages the rows take, at most [`MAX_PAGES`].
    pub pages: usize,
    /// Rows past the last page.
    pub left_out: usize,
    /// Width of the chart panel: the page between its margins, less the list.
    pub panel_w: f64,
    /// Width of each list column, from the widest cell on any page.
    pub col_widths: Vec<f64>,
    /// Font size of the list's text, in px: 11, less when the columns would not all fit whole.
    pub list_font: f64,
    /// The columns the pages print, by header name, in order; `None` is every column the table has.
    pub columns: Option<Vec<String>>,
    /// The dates the pages cover, the half-open span `[from, to)`; `None` prints every row.
    pub window: Option<(Instant, Instant)>,
    /// Instant at the left edge of a page.
    pub t0: f64,
    /// Horizontal scale of every page.
    pub px_per_minute: f64,
}

impl PrintPlan {
    /// The tasks on `page` (0-based), empty past the last page.
    pub fn page_rows(&self, page: usize) -> &[u32] {
        let from = (page * self.rows_per_page).min(self.rows.len());
        let to = (from + self.rows_per_page).min(self.rows.len());
        &self.rows[from..to]
    }
}

impl Session {
    /// The plan for printing: the range's rows if a range is selected, else every visible row.
    /// The scale is set on the viewport fields, so `Viewport::fit`'s five-year clamp does not
    /// apply: a longer plan still fits the panel width.
    pub fn print_plan(&self) -> PrintPlan {
        self.print_plan_on(PageSize::Letter, None, None)
    }

    /// [`Self::print_plan`] for the sheet `size`, printing only the columns `columns` names, in that order; `None` prints every column.
    /// With a `window`, the half-open span `[from, to)` with `from` before `to`, only the rows active in it print ([`active_in`]) and the pages' scale is the window: `t0` is `from`, and a bar that crosses an edge runs past the panel.
    pub fn print_plan_on(
        &self,
        size: PageSize,
        columns: Option<&[String]>,
        window: Option<(Instant, Instant)>,
    ) -> PrintPlan {
        let timeline = self.timeline();
        let dates = self.display_dates();
        let summaries = timeline.summaries();
        let early = |task: u32| {
            summaries
                .get(task)
                .unwrap_or_else(|| (dates[task as usize].es, dates[task as usize].ef))
        };
        let mut all = if self.range_rows().is_some() {
            self.range_tasks()
        } else {
            timeline.order().to_vec()
        };
        if let Some((from, to)) = window {
            all.retain(|&task| active_in(from, to, early(task).0, early(task).1));
        }
        let rows_per_page = PageSize::rows_per_page(timeline.viewport.row_height);
        let pages = all.len().div_ceil(rows_per_page).min(MAX_PAGES);
        let kept = (pages * rows_per_page).min(all.len());
        let rows = all[..kept].to_vec();
        let page_w = size.dims().0;
        let (col_widths, list_w, list_font) =
            widths::column_widths(&self.print_cells(rows.clone(), columns), page_w);
        let panel_w = page_w - 2.0 * MARGIN - list_w;

        let span = window.or_else(|| {
            rows.iter().fold(None, |span: Option<(i64, i64)>, &task| {
                let (es, ef) = early(task);
                Some(span.map_or((es, ef), |(lo, hi)| (lo.min(es), hi.max(ef))))
            })
        });
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        let (start, end) = span.map_or((0.0, 0.0), |(lo, hi)| (lo as f64, hi as f64));
        let (pad, margin) = if window.is_some() {
            (1.0, 0.0)
        } else {
            (1.04, 0.02)
        };
        let span = (end - start).max(timeline::viewport::MIN_SPAN) * pad;
        let px_per_minute = panel_w / span;
        PrintPlan {
            left_out: all.len() - kept,
            rows,
            size,
            rows_per_page,
            panel_w,
            col_widths,
            list_font,
            columns: columns.map(<[String]>::to_vec),
            window,
            pages,
            t0: start - margin * panel_w / px_per_minute,
            px_per_minute,
        }
    }

    /// The draw list of `page` (0-based) of `plan`: the light theme, no hover, selection or range
    /// tint, arrows between rows of the page, and no outlines, overlay or bar labels.
    pub fn print_scene(&self, plan: &PrintPlan, page: usize) -> Scene {
        let timeline = self.timeline();
        let live = &timeline.viewport;
        let order = plan.page_rows(page);
        #[expect(
            clippy::cast_precision_loss,
            reason = "rows_per_page is at most 30 (20 px rows), a small count"
        )]
        let mut view = Viewport::new(
            plan.panel_w,
            live.axis_height + plan.rows_per_page as f64 * live.row_height,
            order.len(),
        );
        view.row_height = live.row_height;
        view.bar_ratio = live.bar_ratio;
        view.bar_radius = live.bar_radius;
        (view.today_dash, view.crit_edge) = (live.today_dash, live.crit_edge);
        view.px_per_minute = plan.px_per_minute;
        view.t0 = plan.t0;

        let theme = Theme::light();
        let mut scene = build_marked(
            &view,
            timeline.dpr,
            order,
            self.display_dates(),
            timeline.summaries(),
            RowMarks::default(),
            None,
            self.backdrop(),
            &theme,
        );
        let links = timeline::scene::LinkIndex::of(self.schedule(), order);
        add_arrows_marked(
            &mut scene,
            &view,
            timeline.dpr,
            self.display_dates(),
            &links,
            None,
            None,
            &self.pin_broken_marks(),
            &theme,
        );
        scene
    }
}
