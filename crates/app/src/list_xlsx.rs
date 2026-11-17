//! The visible list as an Excel sheet part (gs#1875): `xl/worksheets/sheet1.xml` for the `list_csv` rows.

use crate::Session;
use engine::DAY;

/// The style index a date cell carries (`s="1"`): the workbook that holds the sheet must define
/// `cellXfs` entry 1 as the `yyyy-mm-dd hh:mm` format. Entry 0 is the default, so a text cell names no style.
const DATE_STYLE: u32 = 1;

/// Days from 1899-12-30, Excel's day 0, to 1970-01-01.
const EXCEL_DAYS_AT_1970: i64 = 25_569;

/// The first serial Excel counts correctly: 1900-03-01, past its 1900 leap-year error.
const FIRST_SERIAL: i64 = 61;

/// The columns whose cells are a date and a time as the table writes them.
const DATE_COLUMNS: [&str; 4] = ["Start", "Finish", "Actual start", "Actual finish"];

/// `text` as XML character data: `&`, `<`, `>` and `"` escaped, and every character XML 1.0
/// cannot hold (a control character other than tab, line feed and carriage return) dropped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            '\0'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {}
            _ => out.push(c),
        }
    }
    out
}

impl Session {
    /// The worksheet XML of every visible row (after collapse and the row filter, whatever the
    /// range): a header row, then one row per task in table order, in the columns `list_csv`
    /// writes. A date the table shows as `YYYY-MM-DD HH:MM` is a number, an Excel serial day with
    /// the time as its fraction, in style `DATE_STYLE` (1); a date before 1900-03-01 stays text.
    /// Every other cell is an inline string, so a name that starts with `=` is text and no
    /// formula, with no apostrophe added. An empty cell is left out.
    pub fn list_xlsx_sheet(&self) -> String {
        let tasks = self.timeline().order().to_vec();
        let rows = self.rows_cells(tasks, None);
        let data: Vec<String> = (rows.iter().enumerate())
            .map(|(row, cells)| {
                let cells: Vec<String> = (cells.iter().enumerate())
                    .filter_map(|(column, cell)| {
                        let at = format!("{}{}", column_letter(column), row + 1);
                        let date = DATE_COLUMNS.contains(&rows[0][column].as_str()) && row > 0;
                        match date.then(|| self.serial_of(cell)).flatten() {
                            Some(serial) => Some(format!(
                                "<c r=\"{at}\" s=\"{DATE_STYLE}\"><v>{serial}</v></c>"
                            )),
                            None if cell.is_empty() => None,
                            None => Some(format!(
                                "<c r=\"{at}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                                escape(cell)
                            )),
                        }
                    })
                    .collect();
                format!("<row r=\"{}\">{}</row>", row + 1, cells.concat())
            })
            .collect();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{}</sheetData></worksheet>",
            data.concat()
        )
    }

    /// The Excel serial day of a date text `YYYY-MM-DD HH:MM`, or `None` for any other text and
    /// for a date Excel cannot count.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a serial minute count is under 2^53 for any date the table writes"
    )]
    fn serial_of(&self, text: &str) -> Option<f64> {
        let (day, time) = text.split_once(' ')?;
        let (hours, minutes) = time.split_once(':')?;
        let clock = digits(hours)? * 60 + digits(minutes)?;
        let at = self.origin().parse_day(day)?;
        let total = (self.origin().day() + EXCEL_DAYS_AT_1970) * DAY + at + clock;
        (total >= FIRST_SERIAL * DAY).then(|| total as f64 / DAY as f64)
    }
}

/// `part` as a number when it is all ASCII digits.
fn digits(part: &str) -> Option<i64> {
    part.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| part.parse().ok())
        .flatten()
}

/// The letter of spreadsheet column `column` (from 0): the list has under 26 columns.
fn column_letter(column: usize) -> char {
    char::from(b'A' + u8::try_from(column).expect("a list has under 26 columns"))
}
