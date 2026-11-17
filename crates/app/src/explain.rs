//! Why a task starts when it does: the tasks whose dependencies set its early start, then the tasks
//! that set theirs, nearest first. The session is read, never changed.

use crate::{Session, json::quote};
use engine::TaskId;

/// `{"ok":false,"reason":kind,"message":message}`.
pub(crate) fn refuse(kind: &str, message: &str) -> String {
    format!(
        r#"{{"ok":false,"reason":"{kind}","message":{}}}"#,
        quote(message)
    )
}

impl Session {
    /// The chain of tasks behind `task`'s early start, answered as JSON and without changing this
    /// session: its driving predecessors ([`engine::Schedule::driving_predecessors`]) and theirs in
    /// turn, the walk the selection's back path makes. Answers
    /// `{"ok":true,"start":s,"capped":b,"chain":[{"task","id","name","float"}]}` (`start` as the table
    /// writes dates, `float` as it writes total float), or a refusal,
    /// `{"ok":false,"reason":r,"message":m}`, `r` being `busy` or `unknownTask`.
    #[must_use]
    pub fn explain(&self, task: u32) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        if !self.is_live(TaskId(task)) {
            return refuse("unknownTask", &format!("There is no task {task}."));
        }
        let (back, capped) = self.walk_chain(task, true);
        let chain: Vec<String> = back
            .iter()
            .map(|&n| {
                let meta = &self.meta[n as usize];
                let (id, name) = (quote(meta.code_text()), quote(&meta.name));
                let float = quote(&self.task_float(n));
                format!(r#"{{"task":{n},"id":{id},"name":{name},"float":{float}}}"#)
            })
            .collect();
        let start = quote(&self.date_text(self.solution.dates[task as usize].es));
        format!(
            r#"{{"ok":true,"start":{start},"capped":{capped},"chain":[{}]}}"#,
            chain.join(",")
        )
    }
}
