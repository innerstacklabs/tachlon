//! The host API's reads: each answers from the session and changes nothing.

use super::App;
use crate::json::quote;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Ask what moves if tasks slip a week, changing nothing: each of `tasks` (at most 10) is given a
    /// start-no-earlier-than of its early start plus 7 working days, on a copy that is solved, and the
    /// tasks whose early start or finish differ are answered as JSON, or
    /// `{"ok":false,"reason":r,"message":m}`. Reads only.
    pub fn what_moves(&self, tasks: &[u32]) -> String {
        self.session.what_moves(tasks)
    }

    /// Ask what moves if `task` takes `days` working days longer, changing nothing: a copy with the
    /// task's duration grown is solved, and answered like `what_moves`. Reads only.
    pub fn what_moves_longer(&self, task: u32, days: u32) -> String {
        self.session.what_moves_longer(task, days)
    }

    /// Ask why `task` starts when it does, changing nothing: the tasks whose dependencies set its
    /// early start and theirs in turn, nearest first, with their names, activity IDs and total float,
    /// answered as JSON, or `{"ok":false,"reason":r,"message":m}`. Reads only.
    pub fn explain(&self, task: u32) -> String {
        self.session.explain(task)
    }

    /// The first 50 tasks, with number, activity ID and name, behind each `checks` count that walks tasks, as JSON. Reads only.
    pub fn health(&self) -> String {
        self.session.health()
    }

    /// The critical tasks and the longest path's tasks, each the first 50 with number, activity ID and name and the true count, as JSON. Reads only.
    pub fn critical_path(&self) -> String {
        self.session.critical_path()
    }

    /// Whether a bar press, a bar drag or a link drag is in progress: the edits, the row commands
    /// and the outline levels wait for it.
    pub fn busy(&self) -> bool {
        self.session.busy()
    }

    /// The day the baseline was set at, written `YYYY-MM-DD`; empty with no baseline or one set
    /// with no data date. Reads only.
    pub fn baseline_day(&self) -> String {
        self.session.baseline_data_day().unwrap_or_default()
    }

    /// Task `task` as an event names it, as JSON: `{"key":k,"start":s}`, with `k` the key the host
    /// gave the task or `null`, and `s` its early start as `YYYY-MM-DDTHH:mm`. `undefined` for a slot
    /// the session does not hold. The slot lasts until the next load, open or remount. Reads only.
    pub fn task_ref(&self, task: u32) -> Option<String> {
        let at = task as usize;
        let start = self.session.solution().dates.get(at)?.es;
        let key = self.session.meta.get(at)?.key.as_deref();
        Some(format!(
            r#"{{"key":{},"start":{}}}"#,
            key.map_or_else(|| "null".into(), quote),
            quote(&self.session.date_text(start).replacen(' ', "T", 1))
        ))
    }
}
