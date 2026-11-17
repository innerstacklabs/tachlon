//! The edge note's jump.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Pan to the work `status.offscreen` names: with `later`, the next start a fifth in from the
    /// left; otherwise the last finish a fifth in from the right. The rows and the zoom stay and
    /// nothing is selected. False, and nothing changes, when the window has work, that side has
    /// none, or a bar press or drag is in progress.
    pub fn go_to_offscreen(&mut self, later: bool) -> bool {
        self.session.go_to_offscreen(later)
    }
}
