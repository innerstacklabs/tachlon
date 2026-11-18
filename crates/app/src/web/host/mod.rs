//! The host API (gs#1195): edit commands, proposals and reads, each answering with JSON instead of throwing, so
//! the page hands a host a named reason and, beside it, the sentence a person sees: `EditError`'s Display, the
//! one wording table the chart's own alert line also shows. The exports the chart's own controls call
//! (`insert_task`, `set_baseline` and the rest) keep throwing that sentence; the `try_` exports run the same
//! session calls. `edits`, `proposals` and `reads` hold one kind each; this file holds what they share.

mod before_edit;
mod edits;
mod keys;
mod proposals;
mod read_only;
mod reads;

use super::App;
use crate::edit::EditError;
use crate::json::quote;
use crate::texts::{Texts, Worded};
use wasm_bindgen::prelude::*;

/// `{"ok":true}`, with `"task":n` when there is one, or
/// `{"ok":false,"reason":"<EditError::kind>","message":"<EditError's Display>"}`.
fn answer(result: Result<Option<u32>, EditError>, texts: &Texts) -> String {
    match result {
        Ok(None) => r#"{"ok":true}"#.to_string(),
        Ok(Some(task)) => format!(r#"{{"ok":true,"task":{task}}}"#),
        Err(error) => format!(
            r#"{{"ok":false,"reason":"{}","message":{}}}"#,
            error.kind(),
            quote(&error.text(texts))
        ),
    }
}

impl App {
    /// A refusal as a thrown error, in the words of this chart's locale.
    pub(super) fn thrown(&self, error: &impl Worded) -> JsError {
        JsError::new(&self.session.say(error))
    }

    /// The busy refusal as a thrown error, the one place its sentence is written for the throwing
    /// exports.
    pub(super) fn busy_error(&self) -> JsError {
        self.thrown(&EditError::Busy)
    }
}

/// The answer the page gives for a refusal it makes before asking the app: `badDataDate` for a day
/// that is not one, `readOnly`, `busy` (a cell edit, or a switch to WebGL2). The same JSON as a `try_` export, so
/// its `message` comes from the same table. Any other `kind` is not read as busy: it is passed through.
#[wasm_bindgen]
pub fn refusal(kind: &str) -> String {
    answer(
        Err(match kind {
            "badDataDate" => EditError::BadDataDate,
            "busy" => EditError::Busy,
            "readOnly" => EditError::ReadOnly,
            _ => return format!(r#"{{"ok":false,"reason":{0},"message":{0}}}"#, quote(kind)),
        }),
        &Texts::default(),
    )
}
