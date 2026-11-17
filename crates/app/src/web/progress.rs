//! The progress cells' edit entry: one of a task's actual start, actual finish and remaining
//! duration, typed in the table (gs#703).

use super::App;
use engine::TaskId;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Set one progress field of task `i` from the text its cell shows, keeping the other two, as
    /// one undo step: `field` is `actual_start`, `actual_finish`, `remaining` or `percent`; a date
    /// is `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`, a remaining a duration as `set_duration` takes it,
    /// a percent a whole number from 0 to 100 with an optional `%` (it sets all three fields);
    /// empty text clears a date or the remaining. Throws `JsError`, changing nothing, for another
    /// `field`, an unknown task, text that does not parse, a finish before the start, a summary,
    /// or a bar press.
    pub fn set_progress_field(&mut self, i: u32, field: &str, text: &str) -> Result<(), JsError> {
        if field == "percent" {
            return self
                .session
                .set_percent_text(TaskId(i), text)
                .map_err(|error| self.thrown(&error));
        }
        if !matches!(field, "actual_start" | "actual_finish" | "remaining") {
            return Err(JsError::new(&format!("Unknown progress field: {field}.")));
        }
        self.session
            .set_progress_field_text(TaskId(i), field, text)
            .map_err(|error| self.thrown(&error))
    }
}
