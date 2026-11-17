//! The baseline's "Add new" and "Re-baseline" entries and the day it was set at (gs#703).

use super::App;
use engine::TaskId;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Give the tasks added since the baseline their current dates as baseline, keeping every other
    /// entry and every recorded slip. Returns the number of tasks it added. One undo step. Throws
    /// `JsError`, changing nothing, with no baseline or during a bar press.
    pub fn add_new_to_baseline(&mut self) -> Result<u32, JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        let before = self.baseline_rows();
        self.session
            .run_add_new_to_baseline()
            .map_err(|error| self.thrown(&error))?;
        Ok(self.baseline_rows() - before)
    }

    /// Reset task `i`'s baseline entry to its current dates, keeping every other entry. One undo
    /// step. Throws `JsError`, changing nothing, when the task cannot be reset or during a bar press.
    pub fn rebaseline_task(&mut self, i: u32) -> Result<(), JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        self.session
            .run_rebaseline_task(TaskId(i))
            .map_err(|error| self.thrown(&error))
    }

    /// The day the baseline was set at, written `14 Oct`; empty with no baseline or one set with no
    /// data date. Reads only.
    pub fn baseline_data_date(&self) -> String {
        self.session.baseline_data_date_text().unwrap_or_default()
    }
}
