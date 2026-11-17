//! The shapes that tell the data date from the critical path without colour (tl#1215).

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Set the shape look from the `--gantt-today-dash`, `--gantt-today-gap` and
    /// `--gantt-crit-edge-w` tokens, in CSS pixels. A dash under 1, a gap or edge under 0, or a NaN
    /// throws and changes nothing.
    pub fn set_shape_look(&mut self, dash: f64, gap: f64, edge: f64) -> Result<(), JsError> {
        self.session
            .set_shape_look(dash, gap, edge)
            .map_err(|error| JsError::new(&error))
    }
}
