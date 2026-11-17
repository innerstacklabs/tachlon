//! A task's `Notes` text: the reader does not carry it, and says so once (gs#703).

use crate::dom::Node;
use crate::notes::Notes;

/// Records the one sentence when the task has a non-empty `Notes` element. [`Notes::line`] keeps
/// a repeated sentence once, so any number of tasks with notes lists it a single time.
pub(super) fn note_dropped(node: &Node, notes: &mut Notes) {
    if node
        .child_text("Notes")
        .is_some_and(|value| !value.is_empty())
    {
        notes.line("Task notes were not read; no dates are affected.".to_owned());
    }
}
