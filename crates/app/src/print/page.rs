//! One printed page as SVG (gs#703): a header, the list columns on the left, the page scene on
//! the right and, on the last page, a line for the rows the plan left out.

use super::widths::{DATE_COLUMNS, SHORT_HEADS, text_px};
use super::{FOOTER_H, HEADER_H, MARGIN, PageSize, PrintPlan};
use crate::Session;
use crate::texts as t;
use engine::{Instant, TaskId};
use std::fmt::Write;

pub(super) const FONT: f64 = 11.0;
const INDENT: f64 = 12.0;

pub(super) fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Clear space at the right of a cell's clip, so a clipped name or date stops short of the next cell's text.
pub(super) const GUTTER: f64 = 4.0;
/// A cell the list's width leaves less room than this at [`FONT`] (less at a smaller size) is clipped to nothing: its text would be a sliver.
const MIN_CELL: f64 = 20.0;

/// `body` as printed in `room` px, `width(text, size)` being a text's width in px at `size`: whole when it fits, else cut short and ending in an ellipsis.
fn fit(body: &str, room: f64, size: f64, width: &dyn Fn(&str, f64) -> f64) -> String {
    if width(body, size) <= room + 1e-6 {
        return body.to_string();
    }
    let cut = |n: usize| body.chars().take(n).chain(['…']).collect::<String>();
    (0..body.chars().count())
        .rev()
        .find(|&n| width(&cut(n), size) <= room + 1e-6)
        .map_or_else(String::new, cut)
}

/// [`fit`] for a code whose last part tells rows apart: a cut keeps the end and puts the ellipsis first.
fn fit_end(body: &str, room: f64, size: f64, width: &dyn Fn(&str, f64) -> f64) -> String {
    let reversed: String = body.chars().rev().collect();
    fit(&reversed, room, size, width).chars().rev().collect()
}

fn text(out: &mut String, x: f64, y: f64, size: f64, anchor: &str, body: &str) {
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{y}\" font-size=\"{size}\"{anchor} fill=\"#1f2328\">{}</text>",
        esc(body)
    );
}

impl Session {
    /// A list head in the host's words, or unchanged when it is not one of the built-in heads.
    fn print_head(&self, head: &str) -> String {
        let text = match head {
            "ID" => &t::HEAD_ID,
            "WBS" => &t::HEAD_WBS,
            "Task" => &t::HEAD_TASK,
            "Dur" => &t::HEAD_DUR,
            "Start" => &t::HEAD_START,
            "Finish" => &t::HEAD_FINISH,
            "Float" => &t::HEAD_FLOAT,
            "Free float" => &t::HEAD_FREE_FLOAT,
            "Start variance" => &t::HEAD_START_VARIANCE,
            "Finish variance" => &t::HEAD_FINISH_VARIANCE,
            "Float variance" => &t::HEAD_FLOAT_VARIANCE,
            "Actual start" => &t::HEAD_ACTUAL_START,
            "Actual finish" => &t::HEAD_ACTUAL_FINISH,
            "Remaining" => &t::HEAD_REMAINING,
            "% complete" => &t::HEAD_PERCENT_COMPLETE,
            "Start var." => &t::HEAD_START_VAR,
            "Finish var." => &t::HEAD_FINISH_VAR,
            "Float var." => &t::HEAD_FLOAT_VAR,
            _ => return head.to_string(),
        };
        self.texts.say(text, &[])
    }

    /// [`Session::rows_cells`] as printed: a name's tabs and line breaks read as spaces, and a date column holds the day alone.
    pub(super) fn print_cells(
        &self,
        tasks: Vec<u32>,
        columns: Option<&[String]>,
    ) -> Vec<Vec<String>> {
        let mut table = self.rows_cells(tasks, columns);
        let dates: Vec<bool> = table[0]
            .iter()
            .map(|h| DATE_COLUMNS.contains(&h.as_str()))
            .collect();
        for (r, row) in table.iter_mut().enumerate() {
            for (cell, &date) in row.iter_mut().zip(&dates) {
                *cell = cell.replace(['\t', '\n', '\r'], " ");
                if date && r > 0 {
                    cell.truncate(cell.find(' ').unwrap_or(cell.len()));
                }
                if let (0, Some((_, short))) =
                    (r, SHORT_HEADS.iter().find(|(long, _)| cell == long))
                {
                    *cell = (*short).to_string();
                }
            }
        }
        table
    }

    /// Page `page` (0-based) of `plan` as one `<svg>` the size of its sheet.
    #[expect(
        clippy::too_many_lines,
        clippy::too_many_arguments,
        reason = "one page, from table text to svg, and the page's own header words"
    )]
    fn print_page(
        &self,
        plan: &PrintPlan,
        page: usize,
        pages: usize,
        title: &str,
        printed_on: &str,
        layout: Option<&str>,
        measure: &dyn Fn(&str, f64) -> f64,
    ) -> String {
        let rows = plan.page_rows(page);
        let table = self.print_cells(rows.to_vec(), plan.columns.as_deref());
        let mut lines = table
            .iter()
            .map(|line| line.iter().map(String::as_str).collect::<Vec<_>>());
        let head = lines.next().unwrap_or_default();
        let widths = &plan.col_widths;
        let (page_w, page_h) = plan.size.dims();
        let list_w = page_w - 2.0 * MARGIN - plan.panel_w;
        let task_col = head.iter().position(|&name| name == "Task");
        let wbs_col = head.iter().position(|&name| name == "WBS");
        let worded_head = head
            .iter()
            .map(|head| self.print_head(head))
            .collect::<Vec<_>>();
        let worded_head = worded_head.iter().map(String::as_str).collect::<Vec<_>>();

        let scene = self.print_scene(plan, page);
        let live = &self.timeline().viewport;
        let (axis, row_h) = (live.axis_height, live.row_height);
        // The list and the panel fill the page's width, so only a taller row can shrink the body.
        #[expect(
            clippy::cast_precision_loss,
            reason = "rows_per_page is at most ROWS_PER_PAGE, a small constant"
        )]
        let body_h = axis + plan.rows_per_page as f64 * row_h;
        let scale = ((page_h - 2.0 * MARGIN - HEADER_H - FOOTER_H) / body_h).min(1.0);

        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {page_w} {page_h}\" font-family=\"sans-serif\">"
        );
        let _ = write!(
            out,
            "<rect width=\"{page_w}\" height=\"{page_h}\" fill=\"#ffffff\"/>"
        );
        text(&mut out, MARGIN, MARGIN + 14.0, 16.0, "", title);
        let mut sub = Vec::new();
        let say = |text, holes: &[(&str, &str)]| self.texts.say(text, holes);
        if let Some(date) = self.data_date_text() {
            sub.push(say(&t::PRINT_DATA_DATE, &[("date", &date)]));
        }
        if let Some(name) = layout.filter(|name| !name.trim().is_empty()) {
            sub.push(say(&t::PRINT_LAYOUT, &[("name", name)]));
        }
        let filter = self.row_filter();
        if filter != crate::row_filter::RowFilter::Off {
            sub.push(say(
                &t::PRINT_FILTER,
                &[("filter", &filter.print_label(&self.texts))],
            ));
        }
        if let Some(window) = self.lookahead() {
            let text = if window.weeks == 1 {
                &t::PRINT_LOOKAHEAD_ONE
            } else {
                &t::PRINT_LOOKAHEAD_MANY
            };
            let (start, weeks) = (self.lookahead_start_text(), window.weeks.to_string());
            sub.push(say(text, &[("start", &start), ("weeks", &weeks)]));
        }
        if let Some((from, to)) = plan.window {
            let (a, b) = (self.origin.day_text(from), self.origin.day_text(to));
            sub.push(say(&t::PRINT_DATES, &[("from", &a), ("to", &b)]));
        }
        let right = page_w - MARGIN;
        let printed = say(&t::PRINT_PRINTED, &[("date", printed_on)]);
        let end = if printed_on.is_empty() {
            right
        } else {
            right - text_px(&printed) - GUTTER
        };
        let sub = fit(&sub.join("   "), end - MARGIN, FONT, &|b, _| text_px(b));
        text(&mut out, MARGIN, MARGIN + 32.0, FONT, "", &sub);
        let numbers = [(page + 1).to_string(), pages.to_string()];
        let label = say(
            &t::PRINT_PAGE,
            &[("page", &numbers[0]), ("pages", &numbers[1])],
        );
        text(
            &mut out,
            right,
            MARGIN + 14.0,
            FONT,
            " text-anchor=\"end\"",
            &label,
        );
        if !printed_on.is_empty() {
            text(
                &mut out,
                right,
                MARGIN + 32.0,
                FONT,
                " text-anchor=\"end\"",
                &printed,
            );
        }

        let _ = write!(
            out,
            "<g transform=\"translate({MARGIN} {}) scale({scale})\">",
            MARGIN + HEADER_H
        );
        let estimate = |body: &str, size: f64| text_px(body) * size / FONT; // sizes a value's column, so it cuts a value; only a name is measured
        let cell_at = |out: &mut String, id: &mut usize, y: f64, line: &[&str], depth: u32| {
            let mut x = 0.0;
            for (c, (&width, cell)) in widths.iter().zip(line).enumerate() {
                let w = width.min(list_w - x).max(0.0);
                let clip = if w < MIN_CELL * plan.list_font / FONT {
                    0.0
                } else {
                    w - GUTTER
                };
                let pad = if Some(c) == task_col {
                    f64::from(depth) * INDENT
                } else {
                    0.0
                };
                let _ = write!(
                    out,
                    "<clipPath id=\"pc{id}\"><rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"{row_h}\"/></clipPath><g clip-path=\"url(#pc{id})\">",
                    y - row_h + 4.0,
                    clip
                );
                let task = Some(c) == task_col;
                let by: &dyn Fn(&str, f64) -> f64 = if task { measure } else { &estimate };
                let fit = if Some(c) == wbs_col { fit_end } else { fit };
                let shown = fit(cell, clip - 2.0 - pad, plan.list_font, by);
                text(out, x + 2.0 + pad, y, plan.list_font, "", &shown);
                out.push_str("</g>");
                *id += 1;
                x += width;
            }
        };
        let mut id = 0;
        cell_at(&mut out, &mut id, axis - 6.0, &worded_head, 0);
        for (i, (&task, line)) in rows.iter().zip(lines).enumerate() {
            let depth = self
                .outline()
                .map_or(0, |outline| outline.depth(TaskId(task)) - 1);
            #[expect(
                clippy::cast_precision_loss,
                reason = "i is a row number on one page, at most ROWS_PER_PAGE, exact in f64"
            )]
            cell_at(
                &mut out,
                &mut id,
                axis + (i as f64 + 1.0) * row_h - 6.0,
                &line,
                depth,
            );
        }
        // A nested `svg` clips to the panel: a window's bars cross its edge.
        let _ = write!(
            out,
            "<svg x=\"{list_w}\" width=\"{}\" height=\"{body_h}\">{}{}</svg></g>",
            plan.panel_w,
            timeline::scene::svg(&scene),
            self.month_names(plan, axis)
        );

        super::legend::add_legend(
            &mut out,
            &self.texts,
            MARGIN,
            page_h - MARGIN - FOOTER_H + 4.0,
        );
        if page + 1 == pages && plan.left_out > 0 {
            let count = plan.left_out.to_string();
            let note = say(&t::PRINT_LEFT_OUT, &[("count", &count)]);
            text(&mut out, MARGIN, page_h - MARGIN, FONT, "", &note);
        }
        out.push_str("</svg>");
        out
    }

    /// Every page of the print plan as an SVG string, in page order (at least one). `printed_on`
    /// is drawn as "Printed `{printed_on}`" under the page number; empty draws nothing.
    pub fn print_pages(&self, title: &str, printed_on: &str) -> Vec<String> {
        self.print_pages_on(title, printed_on, PageSize::Letter, None, None, None)
    }

    /// [`Self::print_pages`] on the sheet `size`, printing the list columns `columns` names (`None` prints every column) and only the rows active in `window` (`None` prints every row; see [`Self::print_plan_on`]).
    /// A `layout` name, when it is not empty, heads every page as `Layout: <name>`.
    pub fn print_pages_on(
        &self,
        title: &str,
        printed_on: &str,
        size: PageSize,
        columns: Option<&[String]>,
        window: Option<(Instant, Instant)>,
        layout: Option<&str>,
    ) -> Vec<String> {
        let estimate = |body: &str, size: f64| text_px(body) * size / FONT;
        self.print_pages_measured(title, printed_on, size, &estimate, columns, window, layout)
    }

    /// [`Self::print_pages_on`] with a name cut only when `measure` says it overflows its column: the page's own font, measured, not an estimate.
    /// `columns` names the list columns to print, in order; `None` prints every column the table has. `window` is the dates to print, as in [`Self::print_plan_on`].
    /// `layout` is the name of the print layout in use, as in [`Self::print_pages_on`].
    #[expect(
        clippy::too_many_arguments,
        reason = "the page's own header words and print choices, as `App.print_pages` takes them"
    )]
    pub fn print_pages_measured(
        &self,
        title: &str,
        printed_on: &str,
        size: PageSize,
        measure: &dyn Fn(&str, f64) -> f64,
        columns: Option<&[String]>,
        window: Option<(Instant, Instant)>,
        layout: Option<&str>,
    ) -> Vec<String> {
        let plan = self.print_plan_on(size, columns, window);
        let pages = plan.pages.max(1);
        (0..pages)
            .map(|page| self.print_page(&plan, page, pages, title, printed_on, layout, measure))
            .collect()
    }
}
