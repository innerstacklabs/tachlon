//! Move the selected row among its siblings (gs#703).

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Move the selected row and its subtree above the sibling before it; one undo step. Throws
    /// `JsError` with the reason, changing nothing, when refused (see `Session::move_selected_up`).
    pub fn move_selected_up(&mut self) -> Result<(), JsError> {
        self.session
            .move_selected_up()
            .map_err(|error| self.thrown(&error))?;
        Ok(())
    }

    /// Move the selected row and its subtree below the sibling after it; the same step and refusals.
    pub fn move_selected_down(&mut self) -> Result<(), JsError> {
        self.session
            .move_selected_down()
            .map_err(|error| self.thrown(&error))?;
        Ok(())
    }
}
