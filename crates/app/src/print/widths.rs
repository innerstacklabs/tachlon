//! The widths of the printed list's columns (gs#703): how wide each column is, from the text it holds, and how big that text is set.

use super::MARGIN;
use super::page::{FONT, GUTTER};

/// A name column is never squeezed below this, whatever the other columns take.
const MIN_TASK: f64 = 180.0;
/// WBS shrinks to this first, then ID to [`MIN_ID`], before the list grows.
const MIN_WBS: f64 = 80.0;
const MIN_ID: f64 = 56.0;

/// Width of `body` in px at `FONT`, from em widths on the wide side of Arial and `DejaVu` Sans.
pub(super) fn text_px(body: &str) -> f64 {
    let ems: f64 = body
        .chars()
        .map(|c| match c {
            ' ' | '-' | ':' | '.' | ',' => 0.4,
            'm' | 'w' | 'M' | 'W' => 0.97,
            '…' => 1.0,
            _ => 0.64,
        })
        .sum();
    ems * FONT
}

/// The variance heads as printed, short so a page with every column on keeps them whole.
pub(super) const SHORT_HEADS: [(&str, &str); 3] = [
    ("Start variance", "Start var."),
    ("Finish variance", "Finish var."),
    ("Float variance", "Float var."),
];

/// The timeline panel keeps at least this much of the page between its margins, whatever the columns need.
const MIN_PANEL: f64 = 120.0;
/// At the smallest text ([`MIN_LIST_FONT`]) the panel may give up more, to keep every column whole.
const FLOOR_PANEL: f64 = 48.0;
/// The list's text is never set smaller than this many px (about 7 pt), however many columns it holds.
const MIN_LIST_FONT: f64 = 9.0;
/// [`MIN_LIST_FONT`] as a share of [`FONT`].
const MIN_SCALE: f64 = MIN_LIST_FONT / FONT;
/// The date columns, whose cells print as a day (`2026-10-05`), without the time of day the screen shows.
pub(super) const DATE_COLUMNS: [&str; 4] = ["Start", "Finish", "Actual start", "Actual finish"];

/// The columns of `table` (header first) on a page `page_w` wide: [`widths_at`] for the largest text, down to [`MIN_SCALE`] of
/// [`FONT`], that keeps every column whole, and that font size. Only the columns past the smallest text are cut.
pub(super) fn column_widths(table: &[Vec<String>], page_w: f64) -> (Vec<f64>, f64, f64) {
    let mut scale = 1.0;
    loop {
        let (widths, list) = widths_at(table, page_w, scale);
        if scale <= MIN_SCALE || widths.iter().sum::<f64>() <= list + 1e-6 {
            return (widths, list, FONT * scale);
        }
        scale = (scale - 0.02).max(MIN_SCALE);
    }
}

/// Width of each column of `table` (header first) on a page `page_w` wide, and the width of the
/// list, with the text set at `scale` of [`FONT`]. Every column but the name is as wide as its widest printed cell, its head included, so no
/// value or head is cut; the name column takes what the list's room leaves after them, 200 at most
/// and [`MIN_TASK`] at least. The list has half the page between its margins; when the columns and a
/// name need more, WBS shrinks, then ID, then the list grows until the panel is down to [`MIN_PANEL`].
/// Only the columns past that are cut.
fn widths_at(table: &[Vec<String>], page_w: f64, scale: f64) -> (Vec<f64>, f64) {
    let head = table.first().map_or(&[][..], Vec::as_slice);
    let widest = |c: usize| {
        let cells = table.iter().filter_map(|row| row.get(c));
        cells.map(|cell| text_px(cell)).fold(0.0, f64::max) * scale + 2.0 + GUTTER
    };
    let mut widths: Vec<f64> = head
        .iter()
        .enumerate()
        .map(|(c, name)| {
            if name == "Task" {
                200.0 * scale
            } else {
                widest(c)
            }
        })
        .collect();
    let table_w = page_w - 2.0 * MARGIN;
    let mut cap = table_w / 2.0;
    if let Some(task) = head.iter().position(|name| name == "Task") {
        let others = |w: &[f64]| w.iter().sum::<f64>() - w[task];
        let mut short = others(&widths) + MIN_TASK * scale - cap;
        for (name, min) in [("WBS", MIN_WBS), ("ID", MIN_ID)] {
            if let Some(c) = head.iter().position(|h| h == name) {
                let cut = short.max(0.0).min((widths[c] - min * scale).max(0.0));
                (widths[c], short) = (widths[c] - cut, short - cut);
            }
        }
        let panel = if scale <= MIN_SCALE {
            FLOOR_PANEL
        } else {
            MIN_PANEL
        };
        cap += short.clamp(0.0, table_w - panel - cap);
        widths[task] = (cap - others(&widths)).clamp(MIN_TASK * scale, widths[task]);
    }
    let list = widths.iter().sum::<f64>().min(cap);
    (widths, list)
}

#[cfg(test)]
mod tests {
    use super::column_widths;

    /// Long ID and WBS codes on a Letter page: WBS and ID shrink to their floors, the name keeps its minimum.
    #[test]
    fn column_widths_of_one_table_are_the_ones_printed_before_the_split() {
        let rows = [
            "ID|WBS|Task|Start|Finish|Start var.",
            "ACT-0001-LONG-CODE|HAND.WORK.PACKAGE.LONG.1|Pour|2026-10-05|2026-10-09|3d",
        ];
        let table: Vec<Vec<String>> = rows
            .map(|r| r.split('|').map(String::from).collect())
            .to_vec();
        let (widths, list, font) = column_widths(&table, 1056.0);
        assert_eq!(
            widths.iter().map(|w| format!("{w:.2}")).collect::<Vec<_>>(),
            ["56.00", "80.00", "180.00", "71.12", "71.12", "71.12"]
        );
        assert_eq!((format!("{list:.2}"), font), ("529.36".into(), 11.0));
    }
}
