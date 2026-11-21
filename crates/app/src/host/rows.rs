//! A host removes a task by its key (tl#2087): the row its own data no longer holds. Not an edit: no gate, no
//! undo step, no outbox commit, and `revision` stays, as `update_tasks` keeps it.

use super::update::UpdateError;
use crate::{Session, edit::EditError};
use engine::TaskId;

impl Session {
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
        // A forgotten restore of a deleted row deletes that row for good, so its own steps go too, and so on.
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
        Ok(())
    }
}
