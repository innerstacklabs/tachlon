//! The last find's matches for a host that steps through them (gs#1258).

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// `{"stamp":s,"tasks":[...]}`: the task ids the most recent [`App::find`] kept, in its
    /// order, and `Status.stamp` now ([`crate::Session::found`]). It searches nothing.
    pub fn find_found(&self) -> String {
        let (tasks, stamp) = self.session.found();
        let tasks: Vec<String> = tasks.iter().map(u32::to_string).collect();
        format!(r#"{{"stamp":{stamp},"tasks":[{}]}}"#, tasks.join(","))
    }
}
