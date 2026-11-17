//! The visible list as a CSV file (gs#703): the header line and one line per visible row, in the
//! columns Ctrl+C copies.

use crate::Session;

/// `cell` as a CSV field: in quotes, with each quote doubled, when it holds a comma, a quote or
/// a line break.
fn field(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// A task name as a CSV cell: a name that starts with `=`, `+`, `-` or `@` gets a leading
/// apostrophe, so a spreadsheet reads it as text and does not evaluate it.
fn name_cell(name: &str) -> String {
    if name.starts_with(['=', '+', '-', '@']) {
        format!("'{name}")
    } else {
        name.to_string()
    }
}

impl Session {
    /// Every visible row (after collapse and the row filter, whatever the range) as CSV: a header
    /// line, then one line per row in table order, in the columns `range_text` writes. Lines end
    /// in `\n`; a name keeps its line breaks, inside quotes. A name that starts with `=`, `+`, `-`
    /// or `@` has a leading apostrophe; every other cell (a `-2d` variance, say) is as shown.
    pub fn list_csv(&self) -> String {
        let tasks = self.timeline().order().to_vec();
        let rows = self.rows_cells(tasks, None);
        let name_column = rows[0].iter().position(|cell| cell == "Task");
        rows.iter()
            .map(|cells| {
                cells
                    .iter()
                    .enumerate()
                    .map(|(column, cell)| {
                        if name_column == Some(column) {
                            field(&name_cell(cell))
                        } else {
                            field(cell)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
