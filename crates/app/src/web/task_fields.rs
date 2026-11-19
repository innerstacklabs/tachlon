//! The task dialog's edit entry: a task's changed fields as one undo step (gantt#1875). It runs
//! when the dialog is committed, never per frame.

use super::App;
use crate::edit::fields::TASK_FIELDS;
use engine::TaskId;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Object, Reflect};

#[wasm_bindgen]
impl App {
    /// Apply task `i`'s changed fields as one undo step. `fields` holds the changed ones as text
    /// under `name`, `duration`, `start`, `finish`, `snlt`, `fnet`, `fnlt`, `percent`,
    /// `actual_start`, `actual_finish`, `remaining` and `predecessors`, each as its table cell takes it; limits
    /// take a day or blank to clear. A key it lacks is left alone. Throws
    /// `JsError`, changing nothing, starting with the field it refused (`Duration: ...`).
    pub fn set_task_fields(&mut self, i: u32, fields: &Object) -> Result<(), JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        let texts: Vec<(&str, String)> = TASK_FIELDS
            .iter()
            .filter_map(|&(key, _)| {
                Some((key, Reflect::get(fields, &key.into()).ok()?.as_string()?))
            })
            .collect();
        self.session
            .set_task_fields(TaskId(i), &texts)
            .map_err(|error| JsError::new(&error))
    }
}
