//! A host adds and removes a task, and adds, changes and removes a link, by key (tl#2087): what its own data holds,
//! or no longer holds. None is an edit: no gate, no undo step, no outbox commit, and `revision` stays, as
//! `update_tasks` keeps it.

use super::{KeyTarget, update::UpdateError};
use crate::document::json_read::{Value, parse};
use crate::history::Gone;
use crate::{Session, edit::EditError, json::quote, structure::NEW_TASK};
use engine::{DepId, LinkType, Minutes, Pin, TaskId};
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
        self.forget_blocked(task);
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

    /// Add the link `json` describes: one record of `pred` and `succ` (the keys of its tasks), `kind` (`FS`, `SS`,
    /// `FF` or `SF`, default `FS`), `lag` (working minutes, default 0) and the link's own `key`, if it has one. Not an
    /// edit, as an add of a task is not: no undo step, no commit, and `revision` and both selections stay. A key whose
    /// link a person removed brings that link back with the record's `kind` and `lag`. Every step that adds, restores,
    /// sets or removes a link on the pair is forgotten, and the unsaved list is emptied when any is. Answers the link.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`]: `badJson`, `notUpdatable` or `badValue` with the member, `noKey` or `unknownKey` with the
    /// member `pred`, `succ` or `key`, `duplicateKey` (a live link holds the key, or a removed one on other tasks), or
    /// a person's refusal of the link (`busy`, `alreadyLinked`, `linkCycle`, `linkToSummary`). Nothing changes then.
    pub fn add_link_by_key(&mut self, json: &str) -> Result<DepId, UpdateError> {
        let record = link_record(json)?;
        let (pred, succ) = (
            self.link_end(&record, "pred")?,
            self.link_end(&record, "succ")?,
        );
        let (kind, lag) = link_shape(&record)?;
        let (kind, lag) = (kind.unwrap_or(LinkType::FS), lag.unwrap_or(0));
        let key = record.get("key").and_then(Value::as_str);
        let held = key.and_then(|key| self.host_keys().link(key));
        let taken = held.is_some_and(|dep| {
            let link = self.schedule.dependencies()[dep.0 as usize];
            self.schedule.is_dependency_live(dep) || (link.pred, link.succ) != (pred, succ)
        });
        let refusal = if taken { "duplicateKey" } else { "noKey" };
        if taken || key == Some("") {
            return Err(UpdateError::new(refusal, None, Some("key")));
        }
        let (revision, selected) = (self.revision, self.links.selected);
        let added = match held {
            Some(dep) => self.restore_link_as(dep, kind, lag),
            None => self.run_add_link(pred, succ, kind, lag).map(|_| ()),
        };
        (self.revision, self.links.selected) = (revision, selected);
        added.map_err(refuse)?;
        let dep = self.link_between(pred, succ);
        let dep = dep.ok_or_else(|| UpdateError::new("unknownLink", None, None))?;
        if let (None, Some(key)) = (held, key) {
            self.assign_key(KeyTarget::Link(dep), key).map_err(refuse)?;
        }
        self.forget_link(pred, succ);
        Ok(dep)
    }

    /// Set the `kind` and `lag` of the link `json` names, as `add_link_by_key` reads them, the one a member leaves out
    /// keeping its value. The link is named by its `key` or, with none, by the `pred` and `succ` it runs between. Not
    /// an edit: no undo step, no commit, and `revision` stays. A person's later Undo of a step on the link still runs.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`] as `add_link_by_key`'s, with `unknownKey` for a `key` no link holds and `unknownLink` for
    /// tasks no link joins, and a refusal of the new values (`busy`, `unknownLink`, `schedule`). Nothing changes then.
    pub fn set_link_by_key(&mut self, json: &str) -> Result<(), UpdateError> {
        let record = link_record(json)?;
        let dep = self.link_named(&record)?;
        let (kind, lag) = link_shape(&record)?;
        let link = self.schedule.dependencies()[dep.0 as usize];
        let revision = self.revision;
        let done = self.run_set_link(
            dep,
            kind.unwrap_or(link.kind),
            lag.unwrap_or(link.lag),
            false,
        );
        self.revision = revision;
        done.map(|_| ()).map_err(refuse)
    }

    /// Remove the link `json` names (its `key`, else the `pred` and `succ` it runs between) and free its key. Not an
    /// edit: no undo step, no commit, and `revision` stays; the selected link stays unless it is this one. Every step
    /// on a link between the two tasks is forgotten, and the unsaved list is emptied when any is. A link a person
    /// already removed answers `Ok` and changes nothing, so their Undo still brings it back.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`] as `set_link_by_key`'s, and `busy`; nothing changes then.
    pub fn remove_link_by_key(&mut self, json: &str) -> Result<(), UpdateError> {
        let record = link_record(json)?;
        let dep = self.link_named(&record)?;
        if !self.schedule.is_dependency_live(dep) {
            return Ok(());
        }
        let link = self.schedule.dependencies()[dep.0 as usize];
        let (revision, selected) = (self.revision, self.links.selected);
        self.run_remove_link(dep).map_err(refuse)?;
        (self.revision, self.links.selected) = (revision, selected.filter(|&was| was != dep));
        self.free_link_key(dep);
        self.forget_link(link.pred, link.succ);
        Ok(())
    }

    /// Bring link `dep` back with `kind` and `lag`; a refusal of them takes it out again.
    fn restore_link_as(
        &mut self,
        dep: DepId,
        kind: LinkType,
        lag: Minutes,
    ) -> Result<(), EditError> {
        self.run_restore_link(dep, false)?;
        let set = self.run_set_link(dep, kind, lag, false);
        if set.is_err() {
            let _ = self.run_remove_link(dep);
        }
        set.map(|_| ())
    }

    /// The live task a record's member `member` names by key.
    fn link_end(&self, record: &Value, member: &str) -> Result<TaskId, UpdateError> {
        let key = record.get(member).and_then(Value::as_str);
        let key = key.filter(|key| !key.is_empty());
        let key = key.ok_or_else(|| UpdateError::new("noKey", None, Some(member)))?;
        let unknown = || UpdateError::new("unknownKey", None, Some(member));
        self.task_for_key(key).ok_or_else(unknown)
    }

    /// The live link from `pred` to `succ`.
    fn link_between(&self, pred: TaskId, succ: TaskId) -> Option<DepId> {
        let mut links = self.schedule.live_dependencies();
        links
            .find(|(_, link)| (link.pred, link.succ) == (pred, succ))
            .map(|(dep, _)| dep)
    }

    /// The link a record names: its `key`, a removed link's too, else the live one between its `pred` and `succ`.
    fn link_named(&self, record: &Value) -> Result<DepId, UpdateError> {
        if let Some(key) = record.get("key").and_then(Value::as_str) {
            return (self.host_keys().link(key))
                .ok_or_else(|| UpdateError::new("unknownKey", None, Some("key")));
        }
        let (pred, succ) = (
            self.link_end(record, "pred")?,
            self.link_end(record, "succ")?,
        );
        self.link_between(pred, succ)
            .ok_or_else(|| UpdateError::new("unknownLink", None, None))
    }

    /// Forget every step that adds, restores, sets or removes a link from `pred` to `succ`: the host's data now
    /// says what that link is, and a replay would repeat it or take it back. A forgotten restore or reparent takes the
    /// steps that name its row, as `forget_row` does, and the unsaved list is emptied.
    fn forget_link(&mut self, pred: TaskId, succ: TaskId) {
        let links = self.schedule.dependencies();
        let (forgot, gone) = (self.history).forget(|step| step.names_link(pred, succ, links));
        if forgot > 0 {
            self.host.pending.truncate(0);
        }
        for step in gone {
            match step {
                Gone::Reparent(task) => self.forget_row(task),
                Gone::Restore(task) if self.schedule.is_deleted(task) => self.forget_row(task),
                _ => {}
            }
        }
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

    /// Forget the steps the add of `task` makes impossible, and with a forgotten reparent or restore the steps that
    /// name its row, as `forget_row` does.
    fn forget_blocked(&mut self, task: TaskId) {
        let Some(outline) = self.outline.as_ref() else {
            return;
        };
        let (mut todo, mut rows, mut forgot) = (vec![task], Vec::new(), 0);
        while let Some(row) = todo.pop() {
            let (count, gone) = self
                .history
                .forget(|step| step.blocked_by_add(row, outline, &self.schedule));
            forgot += count;
            for step in gone {
                match step {
                    Gone::Delete(task) => todo.push(task),
                    Gone::Reparent(task) => rows.push(task),
                    Gone::Restore(task) if self.schedule.is_deleted(task) => rows.push(task),
                    Gone::Restore(_) => {}
                }
            }
        }
        if forgot > 0 {
            self.host.pending.truncate(0);
            for row in rows {
                self.forget_row(row);
            }
        }
    }

    /// Forget every undo and redo entry that names `row`. A forgotten restore of a deleted row deletes that row for
    /// good, so its own steps go too, and so on. When any is forgotten, the unsaved list is emptied.
    fn forget_row(&mut self, row: TaskId) {
        let (mut rows, mut done, mut forgot) = (vec![row], Vec::new(), 0);
        while let Some(gone) = rows.pop() {
            done.push(gone);
            let links = self.schedule.dependencies();
            let (count, steps) = self.history.forget(|step| step.names(gone, links));
            forgot += count;
            let deleted = |task: &TaskId| self.schedule.is_deleted(*task) && !done.contains(task);
            rows.extend(steps.into_iter().filter_map(|step| match step {
                Gone::Restore(task) if deleted(&task) => Some(task),
                _ => None,
            }));
        }
        if forgot > 0 {
            self.host.pending.truncate(0);
        }
    }
}

const LINK_MEMBERS: [&str; 5] = ["key", "pred", "succ", "kind", "lag"];

fn refuse(error: EditError) -> UpdateError {
    UpdateError::new(error.kind(), None, None)
}

/// A link record: one object that holds no member but `LINK_MEMBERS`.
fn link_record(json: &str) -> Result<Value<'_>, UpdateError> {
    let record = parse(json).ok().filter(|r| matches!(r, Value::Object(_)));
    let record = record.ok_or_else(|| UpdateError::new("badJson", None, None))?;
    if let Value::Object(members) = &record
        && let Some((name, _)) = members
            .iter()
            .find(|(name, _)| !LINK_MEMBERS.contains(&&**name))
    {
        return Err(UpdateError::new("notUpdatable", None, Some(name)));
    }
    Ok(record)
}

/// The `kind` and `lag` a link record sets, each `None` when it leaves the member out.
fn link_shape(record: &Value) -> Result<(Option<LinkType>, Option<Minutes>), UpdateError> {
    let bad = |member| UpdateError::new("badValue", None, Some(member));
    let kind = match record.get("kind").map(Value::as_str) {
        None => None,
        Some(Some("FS")) => Some(LinkType::FS),
        Some(Some("SS")) => Some(LinkType::SS),
        Some(Some("FF")) => Some(LinkType::FF),
        Some(Some("SF")) => Some(LinkType::SF),
        Some(_) => return Err(bad("kind")),
    };
    let lag = record
        .get("lag")
        .map(|lag| lag.as_int().ok_or_else(|| bad("lag")));
    Ok((kind, lag.transpose()?))
}
