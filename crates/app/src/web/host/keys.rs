//! The host API's key commands: the host hands back the key it gave a task or link the chart made.

use super::{App, answer};
use crate::host::KeyTarget;
use engine::{DepId, TaskId};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Give task slot `slot` the key `key`, answered as JSON: `{"ok":true}` or
    /// `{"ok":false,"reason":r,"message":m}` (`emptyKey`, `hasKey`, `keyTaken`, `unknownTask`). Not an
    /// edit, and not refused while a bar is pressed. The slot lasts until the next load, open or remount.
    pub fn assign_task_key(&mut self, slot: u32, key: &str) -> String {
        let result = self.session.assign_key(KeyTarget::Task(TaskId(slot)), key);
        answer(result.map(|()| None), &self.session.texts)
    }

    /// Give link slot `slot` the key `key`, answered like [`App::assign_task_key`] with `unknownLink`
    /// in place of `unknownTask`.
    pub fn assign_link_key(&mut self, slot: u32, key: &str) -> String {
        let result = self.session.assign_key(KeyTarget::Link(DepId(slot)), key);
        answer(result.map(|()| None), &self.session.texts)
    }

    /// The session slot of each key in `keys`, as a JSON array of the same length: the number of the live task that
    /// holds the key, or `null` for a key no task holds and for a deleted task's. Reads only. The slot lasts until the
    /// next load, open or remount.
    pub fn task_slots(&self, keys: Vec<String>) -> String {
        let slot = |key: String| {
            self.session
                .task_for_key(&key)
                .map_or_else(|| "null".into(), |task| task.0.to_string())
        };
        format!(
            "[{}]",
            keys.into_iter().map(slot).collect::<Vec<_>>().join(",")
        )
    }
}
