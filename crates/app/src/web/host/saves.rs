//! The host API's save commands: the host tells the chart a commit was saved, or that its save failed.

use super::App;
use crate::json::quote;
use crate::texts::Worded;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// The host saved every commit up to `seq`: a rollback to one of them is refused from now on.
    pub fn acknowledge(&mut self, seq: u32) {
        self.session.acknowledge(u64::from(seq));
    }

    /// Put the chart back to before the commit `seq`, answered as JSON: `{"ok":true,"seq":n}` with the
    /// rollback commit's `seq`, or `{"ok":false,"reason":r,"message":m,"snapshot":true,"seq":n}` with the
    /// latest commit's `seq` (`rollbackGone`, `busy`): the host takes a snapshot instead.
    pub fn rollback_to(&mut self, seq: u32) -> String {
        match self.session.rollback_to(u64::from(seq)) {
            Ok(seq) => format!(r#"{{"ok":true,"seq":{seq}}}"#),
            Err(error) => format!(
                r#"{{"ok":false,"reason":"{}","message":{},"snapshot":true,"seq":{}}}"#,
                error.kind(),
                quote(&error.text(&self.session.texts)),
                self.session.host.outbox.seq
            ),
        }
    }
}
