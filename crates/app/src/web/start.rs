//! The Start cell's edit entry: a start-no-earlier-than typed in the table (gs#703). It runs when
//! a cell is committed, never per frame.

use super::App;
use engine::TaskId;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Set task `i`'s start-no-earlier-than from the text its Start cell shows, as one undo step:
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`; empty text removes the constraint. Returns the empty
    /// string after a change, or the status line's note when nothing changed (the links already
    /// pass the date). Throws `JsError`, changing nothing, for a summary, a task with actuals,
    /// text that is not a date, an unknown task or a bar press.
    pub fn set_start_text(&mut self, i: u32, text: &str) -> Result<String, JsError> {
        self.session
            .set_start_text(TaskId(i), text)
            .map(|outcome| outcome.note(&self.session.texts).unwrap_or_default())
            .map_err(|error| self.thrown(&error))
    }

    /// Set task `i`'s duration from the day its Finish cell shows, as one undo step: `YYYY-MM-DD`.
    /// The start stays; the duration becomes the working time from it to the end of that day.
    /// Throws `JsError`, changing nothing, for a summary, a task with an actual start or finish,
    /// text that is not a day, a day before the start's, an unknown task or a bar press.
    pub fn set_finish_text(&mut self, i: u32, text: &str) -> Result<(), JsError> {
        self.session
            .set_finish_text(TaskId(i), text)
            .map_err(|error| self.thrown(&error))
    }
}
