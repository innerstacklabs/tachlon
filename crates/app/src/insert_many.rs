//! Insert many rows from a list of names as one undo step (gs#703): the engine half of paste.

use crate::{
    Session,
    edit::{Edit, EditError},
    history::{Source, Step},
    host::Plan,
};
use engine::TaskId;

impl Session {
    /// Insert one task per name, each below the one before: the first below the selected subtree
    /// (see [`Session::place_below`]), the rest below it, so the rows read in the order given. Each
    /// is a leaf as [`Session::insert_below_selection`] makes one. Names are trimmed. The last new
    /// row is selected. The whole list is one undo step. No names inserts nothing and records
    /// nothing. Returns the new tasks in order.
    ///
    /// # Errors
    /// An [`EditError`] for the first refused name (empty, a control character, `" / "`), while a
    /// row filter or lookahead window is on, or the edit's refusal; nothing changes then.
    pub fn insert_many<S: AsRef<str>>(&mut self, names: &[S]) -> Result<Vec<TaskId>, EditError> {
        let names: Vec<String> = names
            .iter()
            .map(|name| name.as_ref().trim().to_string())
            .collect();
        self.gate(&Plan::Paste(&names))?;
        let place = self.place_below(None);
        for name in &names {
            Edit::Insert {
                place,
                name: name.clone(),
            }
            .check_typed()?;
        }
        let mut inverses = Vec::new();
        let mut tasks = Vec::new();
        for name in names {
            let selected = self.selection.as_ref().map(|s| TaskId(s.task));
            let place = self.place_below(selected);
            let inverse = match self.run(Edit::Insert { place, name }) {
                Ok((inverse, _)) => inverse,
                Err(error) => {
                    for inverse in inverses.into_iter().rev() {
                        let _ = self.run(inverse);
                    }
                    return Err(error);
                }
            };
            if let Edit::Delete { task } = &inverse {
                // With nothing selected a new row is top level, whatever level the last root holds.
                if !selected.is_some_and(|task| self.is_live(task)) {
                    self.meta[task.0 as usize].outline_level = 1;
                }
                tasks.push(*task);
            }
            inverses.push(inverse);
        }
        if !inverses.is_empty() {
            let steps = inverses.into_iter().map(Step::plain).collect();
            self.record_group(steps, Source::Paste);
        }
        Ok(tasks)
    }
}
