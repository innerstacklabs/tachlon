//! A host adds and removes a task by its key (tl#2087): the row its own data holds, or no longer holds. Neither is
//! an edit: no gate, no undo step, no outbox commit, and `revision` stays, as `update_tasks` keeps it.

use super::{KeyTarget, update::UpdateError};
use crate::document::json_read::{Value, parse};
use crate::{Session, edit::EditError, json::quote, structure::NEW_TASK};
use engine::{Pin, TaskId};
use std::fmt::Write as _;

impl Session {
    /// Add the task `json` describes, one record as `update_tasks` takes it (its `key` and the members it sets), under
    /// the task keyed `parent` (none: the parent of `after`, else the top level) and after the one keyed `after`
    /// (none: last under that parent). The row starts as a person's insert makes it, then takes the record's values;
    /// it is taken while a row filter or a grouping is on, and the selection stays. A key whose row a person deleted
    /// brings that row back there, its links with it, and forgets every step that names it, as a remove does: their
    /// old Undo of the delete is gone. Answers the task.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`]: `busy`, `noOutline`, `badJson` (not one object), `noKey`, `duplicateKey` (a live task holds
    /// the key), `unknownKey` with the member `parent` or `after`, a place a person's insert refuses (`badPlace`,
    /// `outline`), or the record's refusal as `update_tasks` answers it, with no `record`. Nothing changes then.
    pub fn add_task_by_key(
        &mut self,
        json: &str,
        parent: Option<&str>,
        after: Option<&str>,
    ) -> Result<TaskId, UpdateError> {
        let refuse = |error: EditError| UpdateError::new(error.kind(), None, None);
        let fail = |kind, member| UpdateError::new(kind, None, Some(member));
        self.check_structural().map_err(refuse)?;
        let record = parse(json).ok().filter(|r| matches!(r, Value::Object(_)));
        let record = record.ok_or_else(|| UpdateError::new("badJson", None, None))?;
        let key = record.get("key").and_then(Value::as_str).unwrap_or("");
        let held = self.host_keys().task(key);
        match held {
            _ if key.is_empty() => return Err(fail("noKey", "key")),
            Some(row) if !self.schedule.is_deleted(row) => return Err(fail("duplicateKey", "key")),
            _ => {}
        }
        let task_of = |member, key: Option<&str>| {
            let task = |key| {
                self.task_for_key(key)
                    .ok_or_else(|| fail("unknownKey", member))
            };
            key.map(task).transpose()
        };
        let place = self.host_place(task_of("parent", parent)?, task_of("after", after)?);
        let revision = self.revision;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the id Schedule::add_task gives the next task, read as insert_below_selection reads it"
        )]
        let (task, done) = match held {
            Some(row) => (row, self.run_restore(row, place, None, false)),
            None => (
                TaskId(self.schedule.tasks().len() as u32),
                self.run_insert(place, NEW_TASK, false),
            ),
        };
        let inverse = done.map_err(refuse)?.0;
        if held.is_none() {
            self.assign_key(KeyTarget::Task(task), key)
                .map_err(refuse)?;
        }
        if let Err(error) = self.update_tasks(&format!("[{json}]")) {
            let _ = self.run_delete(task, inverse.meta);
            if held.is_none() {
                self.free_task_key(task);
            }
            self.revision = revision;
            return Err(UpdateError {
                record: None,
                ..error
            });
        }
        if held.is_some() {
            self.forget_row(task);
        }
        self.revision = revision;
        Ok(task)
    }

    /// Delete the task that holds `key`, as a person's delete does (its links go with it, a selection on it moves
    /// to a neighbor, any other stays), and free the key. Every undo and redo entry that names the row is
    /// forgotten, a whole group that touched it included, since replaying one would be refused and stay on top;
    /// when any is, the unsaved list is emptied too, so a later `rollback_to` refuses and the host reloads. A key
    /// whose row is already deleted (a person's delete, which their Undo can still bring back) answers `Ok` and
    /// changes nothing.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`] of kind `unknownKey` when no task holds `key`, else `busy`, `noOutline` or `hasChildren`
    /// (remove the children first); nothing changes then.
    pub fn remove_task_by_key(&mut self, key: &str) -> Result<(), UpdateError> {
        let Some(row) = self.host_keys().task(key) else {
            return Err(UpdateError::new("unknownKey", None, None));
        };
        if self.schedule.is_deleted(row) {
            return Ok(());
        }
        let revision = self.revision;
        let refuse = |error: EditError| UpdateError::new(error.kind(), None, None);
        self.run_delete(row, None).map_err(refuse)?;
        self.revision = revision;
        self.free_task_key(row);
        self.forget_row(row);
        Ok(())
    }

    /// The members of the task keyed `key` that `update_tasks` reads, as one record it takes back with no change:
    /// `key`, `name`, `duration` (working minutes), `snet` as the task holds it, the limits `snlt`, `fnet` and `fnlt`
    /// and the pins `mso` and `mfo`, each a date to the minute or `null`. `progress` is left out: a percent sets the
    /// actual dates against the plan, so it is no read-back. A summary's record is refused, as any record for it is.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`] of kind `unknownKey` when no live task holds `key`.
    pub fn task_members(&self, key: &str) -> Result<String, UpdateError> {
        let missing = || UpdateError::new("unknownKey", None, None);
        let at = self.task_for_key(key).ok_or_else(missing)?;
        let task = &self.schedule.tasks()[at.0 as usize];
        let (limit, name) = (task.limit, quote(&self.meta[at.0 as usize].name));
        let pin = |finish: bool| match limit.pin {
            Some(Pin::Start(at)) if !finish => Some(at),
            Some(Pin::Finish(at)) if finish => Some(at),
            _ => None,
        };
        let members = [
            ("snet", task.snet),
            ("snlt", limit.snlt),
            ("fnet", limit.fnet),
            ("fnlt", limit.fnlt),
            ("mso", pin(false)),
            ("mfo", pin(true)),
        ];
        let mut dates = String::new();
        for (member, at) in members {
            let _ = write!(dates, r#","{member}":{}"#, self.date(at));
        }
        let (key, duration) = (quote(key), task.duration);
        Ok(format!(
            r#"{{"key":{key},"name":{name},"duration":{duration}{dates}}}"#
        ))
    }

    /// Forget every undo and redo entry that names `row`. A forgotten restore of a deleted row deletes that row for
    /// good, so its own steps go too, and so on. When any is forgotten, the unsaved list is emptied.
    fn forget_row(&mut self, row: TaskId) {
        let (mut rows, mut done, mut forgot) = (vec![row], Vec::new(), 0);
        while let Some(gone) = rows.pop() {
            done.push(gone);
            let links = self.schedule.dependencies();
            let (count, restores) = self.history.forget(|step| step.names(gone, links));
            forgot += count;
            let deleted = |task: &TaskId| self.schedule.is_deleted(*task) && !done.contains(task);
            rows.extend(restores.into_iter().filter(deleted));
        }
        if forgot > 0 {
            self.host.pending.truncate(0);
        }
    }
}
