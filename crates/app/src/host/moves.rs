//! A host moves a task by key (tl#2087): what its own data holds in another place. Not an edit, as the adds and
//! removes in `rows` are not.

use super::update::UpdateError;
use crate::{Session, edit::EditError, reparent::Siblings};

impl Session {
    /// Move the task that holds `key`, with its subtree, under the task keyed `parent` (none: the parent of `after`,
    /// else the top level) and after the one keyed `after` (none: last under that parent); with `first` it is the
    /// first row under that parent instead, as `add_task_by_key` places a row. It is taken while a row filter or a
    /// grouping is on. The selection and the scroll stay, and so do `revision` and the outbox: no gate, no undo step,
    /// no commit. Every undo and redo step that names a row of the moved subtree, or is a reparent with a sibling
    /// count, is forgotten as `remove_task_by_key` forgets them, with the steps the move makes impossible under its
    /// new parent, as `add_task_by_key` does; when any is, the unsaved list is emptied.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`]: `busy`, `noOutline`, `badValue` with the member `after` (`first` with an `after`),
    /// `unknownKey` (no live task holds `key`, or with the member `parent` or `after` for those), or a place a
    /// person's reparent refuses (`badPlace`, `outline`). Nothing changes then.
    pub fn move_task_by_key(
        &mut self,
        key: &str,
        parent: Option<&str>,
        after: Option<&str>,
        first: bool,
    ) -> Result<(), UpdateError> {
        let refuse = |error: EditError| UpdateError::new(error.kind(), None, None);
        let fail = |kind, member| UpdateError::new(kind, None, member);
        self.check_structural().map_err(refuse)?;
        if first && after.is_some() {
            return Err(fail("badValue", Some("after")));
        }
        let task_of = |member, key: Option<&str>| {
            let task =
                |key| (self.task_for_key(key)).ok_or_else(|| fail("unknownKey", Some(member)));
            key.map(task).transpose()
        };
        let row = (self.task_for_key(key)).ok_or_else(|| fail("unknownKey", None))?;
        let place = self.host_place(task_of("parent", parent)?, task_of("after", after)?, first);
        let rows = (self.outline.as_ref()).map_or_else(Vec::new, |o| o.subtree(row).to_vec());
        let revision = self.revision;
        let done = self.run_reparent(row, place, Siblings::Keep, None, true);
        self.revision = revision;
        done.map_err(refuse)?;
        for moved in rows {
            self.forget_row(moved);
        }
        self.forget_blocked(row);
        Ok(())
    }
}
