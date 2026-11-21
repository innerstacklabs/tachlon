//! Forget the history entries that name a task a host removed (tl#2087): an entry that would replay against a
//! deleted row is refused and stays on top, so every older step would be out of reach.

use super::{Entry, History, Step};
use crate::{Place, Siblings, edit::Edit};
use engine::{Dependency, TaskId};

impl Step {
    /// Whether replaying this step needs `row` live: it edits `row`, places a task beside or under it, is a
    /// step of a link to or from it (`links` is every link slot), or is a reparent with a sibling count,
    /// which names no task and so cannot be checked against the outline it was made on.
    pub(crate) fn names(&self, row: TaskId, links: &[Dependency]) -> bool {
        let at = |place: &Place| place.parent == Some(row) || place.after == Some(row);
        let ends = |dep: &engine::DepId| {
            (links.get(dep.0 as usize)).is_some_and(|d| d.pred == row || d.succ == row)
        };
        match &self.edit {
            Edit::Move { task, .. }
            | Edit::SetDuration { task, .. }
            | Edit::Rename { task, .. }
            | Edit::SetProgress { task, .. }
            | Edit::SetLimit { task, .. }
            | Edit::Delete { task } => *task == row,
            Edit::Restore { task, place } => *task == row || at(place),
            Edit::Reparent {
                task,
                place,
                siblings,
            } => *task == row || at(place) || *siblings != Siblings::Keep,
            Edit::Insert { place, .. } => at(place),
            Edit::AddLink { pred, succ, .. } => *pred == row || *succ == row,
            Edit::RemoveLink { dep } | Edit::RestoreLink { dep } | Edit::SetLink { dep, .. } => {
                ends(dep)
            }
            Edit::SetBaseline { .. } | Edit::ClearBaseline | Edit::SetDataDate { .. } => false,
        }
    }
}

impl History {
    /// Drop from both stacks every entry `names` is true of (a group when any of its steps is), and answer how
    /// many were dropped. A host token and a data-date entry stay. The what-if mark moves down by the entries
    /// dropped below it, so a discard still undoes exactly the what-if's own steps.
    pub(crate) fn forget(&mut self, names: impl Fn(&Step) -> bool) -> usize {
        let dropped = |entry: &Entry| match entry {
            Entry::Step(step) => names(step),
            Entry::Group(steps) => steps.iter().any(&names),
            Entry::DataDates(_) | Entry::Host(_) => false,
        };
        let mark = self.whatif.as_ref().and_then(|mark| mark.depth);
        let held = self.undo.len() + self.redo.len();
        let (mut at, mut below) = (0, 0);
        self.undo.retain(|entry| {
            let drop = dropped(entry);
            below += usize::from(drop && mark.is_some_and(|depth| at < depth));
            at += 1;
            !drop
        });
        self.redo.retain(|entry| !dropped(entry));
        if let Some(mark) = &mut self.whatif {
            mark.depth = mark.depth.map(|depth| depth - below);
        }
        held - self.undo.len() - self.redo.len()
    }
}
