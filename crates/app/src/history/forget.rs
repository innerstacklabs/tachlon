//! Forget the history entries a host remove or add makes impossible: a refused replay stays on top,
//! leaving every older step out of reach.

use super::{Entry, History, Step};
use crate::{Place, Siblings, edit::Edit, outline::Outline};
use engine::{Dependency, TaskId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gone {
    Restore(TaskId),
    Reparent(TaskId),
    Delete(TaskId),
}

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

    /// Whether the host's add of `task` (now in `outline`) makes replaying this step impossible, or lets it delete
    /// or release that row: a delete, progress or link step of its parent, which is a summary now, or a counted
    /// reparent whose last children (release) or next siblings (adopt) now include it, or a restore that brings
    /// a link back onto the parent.
    pub(crate) fn blocked_by_add(
        &self,
        task: TaskId,
        outline: &Outline,
        schedule: &engine::Schedule,
    ) -> bool {
        let parent = outline.parent(task);
        match &self.edit {
            Edit::Delete { task } | Edit::SetProgress { task, .. } => Some(*task) == parent,
            Edit::Restore { task: row, .. } => schedule
                .task_links(*row)
                .any(|(_, d)| Some(d.pred) == parent || Some(d.succ) == parent),
            Edit::Reparent {
                task: row,
                siblings: Siblings::Release(n),
                ..
            } => outline
                .children(*row)
                .iter()
                .rev()
                .take(*n)
                .any(|&child| child == task),
            Edit::Reparent {
                task: row,
                siblings: Siblings::Adopt(n),
                ..
            } => {
                let peers = outline
                    .parent(*row)
                    .map_or(outline.roots(), |p| outline.children(p));
                peers
                    .iter()
                    .position(|t| t == row)
                    .is_some_and(|at| peers.iter().skip(at + 1).take(*n).any(|&peer| peer == task))
            }
            Edit::AddLink { pred, succ, .. } => Some(*pred) == parent || Some(*succ) == parent,
            Edit::RemoveLink { dep } | Edit::RestoreLink { dep } | Edit::SetLink { dep, .. } => {
                schedule
                    .dependencies()
                    .get(dep.0 as usize)
                    .is_some_and(|d| Some(d.pred) == parent || Some(d.succ) == parent)
            }
            _ => false,
        }
    }
}

impl History {
    /// Drop from both stacks every entry `names` is true of (a group when any of its steps is), and answer how
    /// many were dropped and every restore, reparent and delete among them. A host token and a data-date entry
    /// stay. The what-if mark moves down by the entries dropped below it, so a discard still undoes its own steps.
    pub(crate) fn forget(&mut self, names: impl Fn(&Step) -> bool) -> (usize, Vec<Gone>) {
        let mut gone = Vec::new();
        let mut dropped = |entry: &Entry| {
            let steps = match entry {
                Entry::Step(step) => std::slice::from_ref(step),
                Entry::Group(steps) => steps,
                Entry::DataDates(_) | Entry::Host(_) => &[],
            };
            let drop = steps.iter().any(&names);
            if drop {
                for step in steps {
                    match step.edit {
                        Edit::Restore { task, .. } => gone.push(Gone::Restore(task)),
                        Edit::Reparent { task, .. } => gone.push(Gone::Reparent(task)),
                        Edit::Delete { task } => gone.push(Gone::Delete(task)),
                        _ => {}
                    }
                }
            }
            drop
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
        (held - self.undo.len() - self.redo.len(), gone)
    }
}
