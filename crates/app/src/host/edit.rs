//! A host's edits to tasks, by key, as one step of the person's own undo history (decision 0019).

use super::update::{UpdateError, Updated};
use crate::{Session, history::Source};

impl Session {
    /// Set the values `json` gives, as [`Session::update_tasks`] reads and sets them, and record the call as
    /// one undo step: one Undo puts back every value, and the person's edit before it is the next Undo. A
    /// call that changes no value records no step. The host's edit is its own, so it passes no gate: a read-only
    /// chart takes it and the before-edit function is not asked. The commit's source is `host`.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`], as `update_tasks` gives, `hasActuals` or `notUndoable`;
    /// the schedule, the history and the outbox are then as they were.
    pub fn host_edit(&mut self, json: &str) -> Result<Updated, UpdateError> {
        let (updated, planned) = self.apply_records(json, true)?;
        let steps: Vec<_> = (planned.iter())
            .flat_map(|(task, (new, old))| new.steps_back(old, *task))
            .collect();
        if !steps.is_empty() {
            self.record_group(steps, Source::Host);
        }
        Ok(updated)
    }
}
