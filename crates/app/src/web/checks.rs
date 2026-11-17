//! The schedule checks report for the Checks panel (gs#703).

use super::App;
use crate::checks::report;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// The schedule checks as one JSON object, text: see [`report::json`] for the keys and for
    /// `invalid_dates` and `execution_index`, which can be `null`. Reads the session and changes
    /// nothing; the page asks when the Checks panel opens, never per frame.
    pub fn checks(&self) -> String {
        report::json(&self.session)
    }
}
