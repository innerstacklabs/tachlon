//! The host API's edit commands, each answering with JSON (see the module above).

use super::{App, answer};
use crate::Session;
use crate::edit::{Edit, EditError};
use crate::host::{BaselineRecordsError, UpdateError, Updated};
use crate::json::quote;
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

    /// `update_tasks`, answered as JSON: `{"ok":true,"updated":n}` (and `"differs":[...]` when a host date differs) or `{"ok":false,"reason":r}` with `record`
    /// (from 0) and `member` when named. Not an edit: no undo step, no commit, `revision` kept; one recompute, or two when the call mixes progress with a duration, start, snet or a limit.
    pub fn update_tasks(&mut self, json: &str) -> String {
        updated_answer(self.session.update_tasks(json))
    }

    /// `host_edit`, answered as `update_tasks` answers, `hasActuals` being one more reason.
    pub fn host_edit(&mut self, json: &str) -> String {
        updated_answer(self.session.host_edit(json))
    }

    /// `add_task_by_key`, answered as JSON: `{"ok":true,"task":n}` or `{"ok":false,"reason":r}` with `member` when
    /// named. Not an edit; a key whose row a person deleted brings it back and forgets their steps that named it.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm_bindgen takes an optional string by value; `Option<&str>` has no ABI"
    )]
    pub fn add_task(
        &mut self,
        json: &str,
        parent: Option<String>,
        after: Option<String>,
    ) -> String {
        match (self.session).add_task_by_key(json, parent.as_deref(), after.as_deref()) {
            Ok(task) => format!(r#"{{"ok":true,"task":{}}}"#, task.0),
            Err(error) => record_refusal(error),
        }
    }

    /// `task_members`, answered as JSON: `{"ok":true,"record":{...}}`, the record `update_tasks` takes back with no
    /// change, or `{"ok":false,"reason":"unknownKey"}`.
    pub fn task_members(&self, key: &str) -> String {
        match self.session.task_members(key) {
            Ok(record) => format!(r#"{{"ok":true,"record":{record}}}"#),
            Err(error) => record_refusal(error),
        }
    }

    /// `remove_task_by_key`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":r}` (`unknownKey`, `hasChildren`,
    /// `busy`, `noOutline`). Not an edit; the person's steps that named the row are forgotten.
    pub fn remove_task(&mut self, key: &str) -> String {
        match self.session.remove_task_by_key(key) {
            Ok(()) => r#"{"ok":true}"#.to_string(),
            Err(error) => record_refusal(error),
        }
    }

    /// `push_host_entry`, answered as JSON: `{"ok":true}` or `{"ok":false,"reason":r}` (`badToken`).
    pub fn try_push_host_entry(&mut self, token: &str) -> String {
        let pushed = self.session.push_host_entry(token);
        answer(pushed.map(|()| None), &self.session.texts)
    }

    /// `clear_history`: both undo stacks, the unsaved list and any what-if are emptied; `seq` is kept.
    pub fn clear_history(&mut self) {
        self.session.clear_history();
    }

    /// `set_baseline_records`, answered as JSON: `{"ok":true}`, a record's refusal as `update_tasks` answers it, or
    /// the edit's as `try_set_baseline` does (`busy`, `readOnly`, `notAllowed`, `refused`).
    pub fn try_set_baseline_records(&mut self, json: &str) -> String {
        if self.session.busy() {
            return answer(Err(EditError::Busy), &self.session.texts);
        }
        match self.session.set_baseline_records(json) {
            Ok(()) => r#"{"ok":true}"#.to_string(),
            Err(BaselineRecordsError::Record(error)) => record_refusal(error),
            Err(BaselineRecordsError::Edit(error)) => answer(Err(error), &self.session.texts),
        }
    }

    /// `set_overlay_records` (null removes it), answered as JSON: `{"ok":true}` or a record's refusal as `update_tasks` answers it.
    /// Not an edit: no busy check, no undo step.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm_bindgen takes an optional string by value; `Option<&str>` has no ABI"
    )]
    pub fn try_set_overlay_records(&mut self, json: Option<String>) -> String {
        match self.session.set_overlay_records(json.as_deref()) {
            Ok(()) => r#"{"ok":true}"#.to_string(),
            Err(error) => record_refusal(error),
        }
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

/// What `update_tasks` and `host_edit` answer.
fn updated_answer(result: Result<Updated, UpdateError>) -> String {
    let done = match result {
        Ok(done) => done,
        Err(error) => return record_refusal(error),
    };
    let list = done.differs.join(",");
    let more = (!list.is_empty()).then(|| format!(r#","differs":[{list}]"#));
    let (tasks, more) = (done.tasks, more.unwrap_or_default());
    format!(r#"{{"ok":true,"updated":{tasks}{more}}}"#)
}

/// `{"ok":false,"reason":"<kind>"}`, with `"record":n` and `"member":m` when the refusal names them.
fn record_refusal(error: UpdateError) -> String {
    let at = (error.record).map_or(String::new(), |n| format!(r#","record":{n}"#));
    let of = (error.member).map_or(String::new(), |m| format!(r#","member":{}"#, quote(&m)));
    format!(r#"{{"ok":false,"reason":"{}"{at}{of}}}"#, error.kind)
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
