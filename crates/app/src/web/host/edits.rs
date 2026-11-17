//! The host API's edit commands, each answering with JSON (see the module above).

use super::{App, answer};
use crate::Session;
use crate::edit::{Edit, EditError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// `insert_task`, answered as JSON: `{"ok":true,"task":n}` with the new task, or
    /// `{"ok":false,"reason":r}`, `r` the refusal's kind (`busy`, `noOutline`, `filterActive`).
    pub fn try_insert_row(&mut self) -> String {
        answer(
            self.session
                .insert_below_selection()
                .map(|task| Some(task.0)),
            &self.session.texts,
        )
    }

    /// `delete_selected` (the selected task, never a selected link), answered as JSON:
    /// `{"ok":true,"task":n}` with the deleted task, or `{"ok":false,"reason":r}` (`noSelection`,
    /// `severalRows`, `hasChildren`, `busy`, `noOutline`).
    pub fn try_delete_row(&mut self) -> String {
        answer(
            self.session.delete_selected().map(|task| Some(task.0)),
            &self.session.texts,
        )
    }

    /// `remove_selected_link`, answered as JSON: `{"ok":true,"link":n}` with the removed link, or
    /// `{"ok":false,"reason":r,"message":m}` (`noLinkSelected`, `busy`).
    pub fn try_delete_link(&mut self) -> String {
        match self.session.remove_selected_link() {
            Ok(dep) => format!(r#"{{"ok":true,"link":{}}}"#, dep.0),
            Err(error) => answer(Err(error), &self.session.texts),
        }
    }

    /// `set_data_date`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":r}` (`badDataDate`,
    /// `busy`).
    pub fn try_set_data_date(&mut self, text: &str) -> String {
        answer(
            self.session.set_data_date_text(text).map(|()| None),
            &self.session.texts,
        )
    }

    /// `set_baseline`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":"busy"}`.
    pub fn try_set_baseline(&mut self) -> String {
        answer(
            self.baseline_edit(|session| session.edit(Edit::SetBaseline { baseline: None })),
            &self.session.texts,
        )
    }

    /// `clear_baseline`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":r}` (`noBaseline`,
    /// `busy`).
    pub fn try_clear_baseline(&mut self) -> String {
        answer(
            self.baseline_edit(|session| session.edit(Edit::ClearBaseline)),
            &self.session.texts,
        )
    }

    /// `add_new_to_baseline`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":r}`
    /// (`noBaseline`, `busy`).
    pub fn try_add_new_to_baseline(&mut self) -> String {
        answer(
            self.baseline_edit(Session::run_add_new_to_baseline),
            &self.session.texts,
        )
    }
}

impl App {
    /// Run a baseline edit after the busy check the throwing exports make (`set_baseline`,
    /// `clear_baseline`, `add_new_to_baseline`).
    fn baseline_edit(
        &mut self,
        run: impl FnOnce(&mut Session) -> Result<(), EditError>,
    ) -> Result<Option<u32>, EditError> {
        if self.session.busy() {
            return Err(EditError::Busy);
        }
        run(&mut self.session).map(|()| None)
    }
}
