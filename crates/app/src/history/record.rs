//! A task as a commit writes it (decision 0019, rule 7): a create's `fields` in the names of
//! decision 0017, and a reference to another task.

use super::changes::LIMIT_FIELDS;
use crate::{Session, json::quote, structure::Place};
use engine::TaskId;

impl Session {
    /// A task as a change names another: its key, else `{ "task": slot }`.
    pub(crate) fn task_ref(&self, task: TaskId) -> String {
        match self
            .meta
            .get(task.0 as usize)
            .and_then(|m| m.key.as_deref())
        {
            Some(key) => quote(key),
            None => format!(r#"{{"task":{}}}"#, task.0),
        }
    }

    /// The reference of a place's task, `null` for none.
    pub(super) fn place_ref(&self, task: Option<TaskId>) -> String {
        task.map_or_else(|| "null".into(), |t| self.task_ref(t))
    }

    /// A live task's fields as a create change writes them, with its place in the outline.
    pub(super) fn fields_of(&self, task: TaskId) -> String {
        let meta = &self.meta[task.0 as usize];
        let outline = self.outline.as_ref().expect("a structure edit ran");
        let place = Place::of(outline, task);
        let text = |value: Option<&str>| value.map_or_else(|| "null".into(), quote);
        let color = meta
            .color
            .map(|[r, g, b, _]| format!("#{r:02x}{g:02x}{b:02x}"));
        let calendar = self.schedule.tasks()[task.0 as usize].calendar;
        let calendar = self
            .host
            .keys
            .calendar_key(calendar)
            .map_or_else(|| format!(r#"{{"calendar":{}}}"#, calendar.0), quote);
        let mut fields = vec![
            format!(r#""parent":{}"#, self.place_ref(place.parent)),
            format!(r#""after":{}"#, self.place_ref(place.after)),
            format!(r#""code":{}"#, text(meta.code.as_deref())),
            format!(r#""wbsCode":{}"#, text(meta.wbs_code.as_deref())),
            format!(r#""calendar":{calendar}"#),
            format!(r#""color":{}"#, text(color.as_deref())),
            format!(r#""label":{}"#, text(meta.label.as_deref())),
        ];
        let own = [
            "name",
            "snet",
            "duration",
            "actualStart",
            "actualFinish",
            "remaining",
        ];
        for field in own.iter().chain(&LIMIT_FIELDS) {
            fields.push(format!(r#""{field}":{}"#, self.value_now(task, field)));
        }
        format!("{{{}}}", fields.join(","))
    }
}
