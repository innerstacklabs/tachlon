//! The print pages as SVG (gs#703).

use super::App;
use crate::PageSize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Every page of the print plan as SVG text, in page order (at least one), `title` at the
    /// top of each and `printed_on` (the day, `YYYY-MM-DD`; empty draws nothing) under the page
    /// number. `page_size` is `"letter"` (also when absent) or `"a4"`, in any case; another
    /// value throws `JsError`. Reads the session and changes nothing, so the live view stays as
    /// it is. `measure(text, size)` is the width of `text` in px, and `columns` names the list columns to print, in order, as the table header
    /// writes them (`docs/WASM_API.md`); absent, every column the table has. `from` and `to` (days,
    /// `YYYY-MM-DD`, both or neither, `to` after `from`; else `JsError`) print only the rows active in `[from, to)`, scaled to it.
    /// `layout`, the name of the print layout in use, heads each page as `Layout: <name>` (absent or empty, nothing).
    #[expect(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        reason = "wasm-bindgen reads the optional JS strings and function into owned values, and the call's arguments are its JS signature"
    )]
    pub fn print_pages(
        &self,
        title: &str,
        printed_on: &str,
        page_size: Option<String>,
        measure: Option<web_sys::js_sys::Function>,
        columns: Option<Vec<String>>,
        from: Option<String>,
        to: Option<String>,
        layout: Option<String>,
    ) -> Result<Vec<String>, JsError> {
        let name = page_size.as_deref().unwrap_or("letter");
        let size = PageSize::parse(name)
            .ok_or_else(|| JsError::new(&format!("unknown page size {name:?}")))?;
        let day = |text: &Option<String>| {
            let text = text.as_deref()?;
            self.session.parse_lookahead_day(text)
        };
        let window = match (&from, &to) {
            (None, None) => None,
            _ => match (day(&from), day(&to)) {
                (Some(from), Some(to)) if from < to => Some((from, to)),
                _ => return Err(JsError::new("from and to are days, to after from")),
            },
        };
        let columns = columns.as_deref();
        let layout = layout.as_deref();
        let Some(measure) = measure else {
            return Ok(self
                .session
                .print_pages_on(title, printed_on, size, columns, window, layout));
        };
        let call = |text: &str, px: f64| {
            let args = (&JsValue::from_str(text), &JsValue::from_f64(px));
            measure.call2(&JsValue::NULL, args.0, args.1).ok()?.as_f64()
        };
        call("…", 1.0)
            .filter(|w| w.is_finite())
            .ok_or_else(|| JsError::new("measure must return a width in px, a finite number"))?;
        let width = |text: &str, px: f64| call(text, px).unwrap_or(f64::INFINITY);
        Ok(self
            .session
            .print_pages_measured(title, printed_on, size, &width, columns, window, layout))
    }
}

#[wasm_bindgen]
impl App {
    /// The print setup as JSON text (`docs/WASM_API.md`): the row filter and Lookahead window the
    /// session has, with the page's `name`, `paper` (`"letter"` or `"a4"`), `columns` (as
    /// `print_pages` takes them; absent, every column) and the date window `from` and `to` (both or
    /// neither). Throws `JsError` for a paper or window `print_pages` would refuse. Reads only.
    pub fn layout_json(
        &self,
        name: &str,
        paper: &str,
        columns: Option<Vec<String>>,
        from: Option<String>,
        to: Option<String>,
    ) -> Result<String, JsError> {
        let window = match (from, to) {
            (None, None) => None,
            (Some(from), Some(to)) => Some((from, to)),
            _ => return Err(JsError::new("from and to are given together")),
        };
        let layout = self.session.layout(name, paper, columns, window);
        layout
            .map(|layout| layout.to_json())
            .map_err(|why| JsError::new(&why))
    }

    /// Apply a layout `layout_json` wrote: set the row filter and the Lookahead window, and return
    /// `{"not_applied":[...]}`, a sentence for every part the schedule could not take (empty when
    /// all went in). The page passes the layout's columns, window and paper to `print_pages` itself.
    /// Throws `JsError` for text that is not a layout.
    pub fn apply_layout(&mut self, layout: &str) -> Result<String, JsError> {
        let layout = crate::layout::Layout::from_json(layout)
            .map_err(|why| JsError::new(&format!("not a layout: {why}")))?;
        let report = self.session.apply_layout(&layout);
        Ok(format!(
            r#"{{"not_applied":{}}}"#,
            crate::json::string_array(&report)
        ))
    }
}
