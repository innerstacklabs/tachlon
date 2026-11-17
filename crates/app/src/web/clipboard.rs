//! The range as clipboard text.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Make every visible row a range, the first row the anchor (`Ctrl+A`). False, changing
    /// nothing, during a bar press or drag or with no rows.
    pub fn select_all_rows(&mut self) -> bool {
        self.session.select_all_rows()
    }

    /// The range's rows, or with no range the selected row, as a header line and one
    /// tab-separated line per row (ID when the file has IDs, WBS when present, Task, Dur, Start,
    /// Finish, Float), or `undefined` with nothing selected.
    pub fn range_text(&self) -> Option<String> {
        self.session.range_text()
    }

    /// Every visible row (collapse and the row filter decide them) as the same text: a header
    /// line and one tab-separated line per row. The page fills the print sheet from it.
    pub fn list_text(&self) -> String {
        self.session.list_text()
    }

    /// Every visible row as CSV (collapse and the row filter decide them, the range does not): a
    /// header line and one line per row, in the columns `range_text` copies, a field quoted when it
    /// holds a comma, a quote or a line break. The page downloads it as `<name>.csv`.
    pub fn list_csv(&self) -> String {
        self.session.list_csv()
    }

    /// Every visible row as the worksheet XML of an Excel file (the rows `list_csv` writes): a date is
    /// a number in date style 1, any other cell an inline string. The page's `xlsxFile` zips it
    /// with an `xl/styles.xml` that defines style 1 (`Gantt.exportXlsx`).
    pub fn list_xlsx_sheet(&self) -> String {
        self.session.list_xlsx_sheet()
    }
}
