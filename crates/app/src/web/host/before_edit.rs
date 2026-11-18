//! The host API's before-edit function: the page's one function, asked before each change a person makes.

use super::App;
use crate::host::{BeforeEdit, Planned};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Function, JSON};

#[wasm_bindgen]
impl App {
    /// Hand the chart the page's before-edit function, or take it away with `None`. It is called once for each
    /// action a person takes, after read-only and before anything changes, with `{ kind, source, tasks, changes }`.
    /// `undefined`, `null` or `true` allows the action, a string refuses it with that reason, and anything else
    /// (`false`, a throw, a promise) refuses it with the default sentence. It must not call the chart: a call into it throws and the change is refused, except that a command answers busy while a cell is being edited.
    pub fn set_before_edit(&mut self, before: Option<Function>) {
        let hand = |f: Function| -> BeforeEdit { Rc::new(move |planned| ask(&f, planned)) };
        self.session.set_before_edit(before.map(hand));
    }
}

fn ask(before: &Function, planned: &Planned) -> Result<(), String> {
    let source = format!("{:?}", planned.source).to_lowercase();
    let json = format!(
        r#"{{"kind":"{}","source":"{source}","tasks":{:?},"changes":[{}]}}"#,
        planned.kind,
        planned.tasks,
        planned.changes.join(",")
    );
    let payload = JSON::parse(&json).map_err(|_| String::new())?;
    match before.call1(&JsValue::UNDEFINED, &payload) {
        Ok(answer) if answer.is_null() || answer.is_undefined() || answer == JsValue::TRUE => {
            Ok(())
        }
        Ok(answer) => Err(answer.as_string().unwrap_or_default()),
        Err(_) => Err(String::new()),
    }
}
