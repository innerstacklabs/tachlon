//! Insert many rows from a list of names (gs#703): the engine half of paste.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Insert one task per name, each below the one before and the first below the selected row's
    /// whole subtree, and select the last. One undo step for the whole list. Returns the new
    /// tasks in order. Throws `JsError` with the reason when refused; nothing changes then.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen reads the JS array of strings into an owned vector"
    )]
    pub fn insert_many(&mut self, names: Vec<String>) -> Result<Vec<u32>, JsError> {
        self.session
            .insert_many(&names)
            .map(|tasks| tasks.into_iter().map(|task| task.0).collect())
            .map_err(|error| self.thrown(&error))
    }
}
