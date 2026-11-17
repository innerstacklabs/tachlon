//! Setting a second file as the baseline (gs#703).

use super::{App, schedule::LoadedSchedule};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Make `other`, a second parsed file such as last month's, the baseline: its tasks are matched
    /// to this session's by ID. Returns the counts and lines as JSON (`CompareReport::json`). One
    /// undo step. Throws `JsError`, changing nothing, when no task has the same ID in both files or
    /// during a bar press.
    pub fn set_baseline_from(&mut self, other: &LoadedSchedule) -> Result<String, JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        self.session
            .set_baseline_from_file(&other.schedule, &other.solution, &other.meta, other.origin)
            .map(|report| self.session.compare_json(&report))
            .map_err(|error| self.thrown(&error))
    }
}
