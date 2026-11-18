//! A host's new values for many tasks, by key, as one recompute (decision 0019, rule 13). It is not an
//! edit: no gate, no undo step, no outbox commit, and `revision` stays; `stamp` rises, so the table
//! redraws. Every record is read before a value is written, and a refusal puts every value back.

use super::read::read_duration;
use crate::{
    Session,
    document::json_read::{Value, parse},
    edit::EditError,
};
use engine::{Instant, Minutes, TaskId};
use std::collections::HashSet;

/// How many tasks an update changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Updated {
    pub tasks: usize,
}

/// Why an update was refused. Nothing changed. `kind` is `badJson`, `noKey`, `unknownKey` (a
/// deleted task's key too), `duplicateKey`, `notUpdatable` (a member an update does not set, or a summary),
/// `badValue`, or an [`EditError::kind`] (`busy`, `schedule`). `record` counts from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateError {
    pub kind: &'static str,
    pub record: Option<usize>,
    pub member: Option<String>,
}

impl UpdateError {
    fn new(kind: &'static str, record: Option<usize>, member: Option<&str>) -> Self {
        let member = member.map(str::to_owned);
        Self {
            kind,
            record,
            member,
        }
    }
}

/// What an update sets on one task.
#[derive(Clone, Copy)]
struct Values {
    duration: Minutes,
    snet: Option<Instant>,
}

impl Session {
    /// Set the durations and starts that `json`, an array of records, gives, each record naming its task
    /// by `key`, and solve once. `duration` is a whole number of working minutes; `start` is held as `snet`
    /// unless the record has an `snet`; `snet` takes a date or `null`. A member a record leaves out keeps
    /// its value. Every other member, the other date limits and the pins included, is `notUpdatable`.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`]; the schedule, the selection, the history and `revision` are then as they were.
    pub fn update_tasks(&mut self, json: &str) -> Result<Updated, UpdateError> {
        let refuse = |error: EditError| UpdateError::new(error.kind(), None, None);
        if self.busy() {
            return Err(refuse(EditError::Busy));
        }
        let bad_json = || UpdateError::new("badJson", None, None);
        let root = parse(json).map_err(|_| bad_json())?;
        let records = root.as_array().ok_or_else(bad_json)?;
        let mut planned = Vec::with_capacity(records.len());
        let mut seen = HashSet::new();
        for (at, record) in records.iter().enumerate() {
            let key = record.get("key").and_then(Value::as_str).unwrap_or("");
            let fail = |kind| UpdateError::new(kind, Some(at), None);
            let task = match self.task_for_key(key) {
                _ if key.is_empty() => return Err(fail("noKey")),
                None => return Err(fail("unknownKey")),
                Some(task) if !seen.insert(task.0) => return Err(fail("duplicateKey")),
                Some(task) => task,
            };
            planned.push((task, self.read_values(at, task, record)?));
        }
        let mut done = Vec::with_capacity(planned.len());
        for &(task, (new, old)) in &planned {
            done.push((task, old));
            if let Err(error) = self.put(task, new) {
                self.put_back(&done);
                return Err(refuse(error));
            }
        }
        let revision = self.revision;
        let solved = self.solve_full(None, |session| session.put_back(&done));
        self.revision = revision;
        solved.map_err(refuse)?;
        if let Some(held) = &mut self.proposal {
            held.solved = None;
        }
        Ok(Updated {
            tasks: planned.len(),
        })
    }

    /// The values `record` gives `task` and the ones it has now.
    fn read_values(
        &self,
        at: usize,
        task: TaskId,
        record: &Value,
    ) -> Result<(Values, Values), UpdateError> {
        let current = &self.schedule.tasks()[task.0 as usize];
        let old = Values {
            duration: current.duration,
            snet: current.snet,
        };
        let members: &[_] = match record {
            Value::Object(members)
                if !self.outline.as_ref().is_some_and(|o| o.is_summary(task)) =>
            {
                members
            }
            _ => return Err(UpdateError::new("notUpdatable", Some(at), Some("key"))),
        };
        let mut new = old;
        for (name, value) in members {
            let bad = || UpdateError::new("badValue", Some(at), Some(name));
            let date = |value: &Value| match value {
                Value::Null => Ok(None),
                _ => self.read_date(value).map(Some).ok_or_else(bad),
            };
            match &**name {
                "key" => {}
                "duration" => new.duration = read_duration(value).ok_or_else(bad)?,
                "start" if record.get("snet").is_some() => {}
                "start" | "snet" => new.snet = date(value)?,
                _ => return Err(UpdateError::new("notUpdatable", Some(at), Some(name))),
            }
        }
        Ok((new, old))
    }

    fn put(&mut self, task: TaskId, values: Values) -> Result<(), EditError> {
        self.schedule.set_duration(task, values.duration)?;
        Ok(self.schedule.set_snet(task, values.snet)?)
    }

    /// Put every task in `done` back, newest first.
    fn put_back(&mut self, done: &[(TaskId, Values)]) {
        for &(task, old) in done.iter().rev() {
            let _ = self.put(task, old);
        }
    }
}
