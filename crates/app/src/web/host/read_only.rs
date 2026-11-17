//! The host API's read-only switch: the host turns the chart read-only for the person using it.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Switch read-only mode. On, every way to change the schedule (a cell, the task dialog, a bar or link
    /// drag, the keyboard, paste, undo and redo) is refused with a sentence that says the chart is
    /// read-only, and a click on a row still selects it. Not an edit; opening another schedule in the chart keeps it.
    pub fn set_read_only(&mut self, on: bool) {
        self.session.set_read_only(on);
    }

    /// Whether the chart is read-only.
    pub fn read_only(&self) -> bool {
        self.session.read_only()
    }
}
