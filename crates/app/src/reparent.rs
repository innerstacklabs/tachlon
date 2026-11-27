//! Reparent (gs#417): move a task with its subtree to a new [`Place`] in the outline, as indent
//! and outdent do. A reparent changes the outline and no task's dates, so like insert and delete
//! it ends in [`Session::rebuild_structure`], which builds everything that can fail before it
//! changes the session.
//!
//! Outdent adopts the siblings below it (ruling R1, as MS Project does): they become the task's
//! last children, and undo hands them back. That is one edit with an exact inverse, so a step
//! carries a [`Siblings`] count.

use crate::edit::{Edit, EditError};
use crate::history::{Meta, Step};
use crate::structure::{Done, Place};
use crate::{Session, meta::TaskMeta};
use engine::{ScheduleError, TaskId};

/// What a reparent does with the task's neighbours besides moving the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Siblings {
    /// Only the task and its subtree move.
    Keep,
    /// The next `n` siblings below the task become its last children, then the whole moves.
    /// Refused when fewer than `n` follow. Inverse: [`Siblings::Release`].
    Adopt(usize),
    /// After the move, the task's last `n` children become its next siblings. Refused when it
    /// has fewer than `n`. Inverse: [`Siblings::Adopt`].
    Release(usize),
}

impl Siblings {
    fn inverse(self) -> Siblings {
        match self {
            Siblings::Keep => Siblings::Keep,
            Siblings::Adopt(n) => Siblings::Release(n),
            Siblings::Release(n) => Siblings::Adopt(n),
        }
    }
}

/// One task's outline fields, kept to put back on a refusal.
type Held = (usize, Option<TaskId>, u8);

impl Session {
    /// Indent the selected row: it becomes the last child of its previous sibling, which opens if
    /// it was collapsed. Recorded for undo; the selection follows the task. Refused, changing
    /// nothing, with no previous sibling, under a row filter or lookahead window, during a bar
    /// press, or when the previous sibling is a leaf with a link (it would become a summary).
    /// Returns the task.
    ///
    /// # Errors
    /// An [`EditError`] for each refusal above, else the edit's refusal.
    ///
    /// # Panics
    /// Panics if the session has no outline. Unreachable: the target check refuses that first.
    pub fn indent_selected(&mut self) -> Result<TaskId, EditError> {
        let task = self.reparent_target()?;
        let outline = self.outline.as_ref().expect("checked");
        let (siblings, at) = siblings_of(outline, task);
        let previous = at
            .checked_sub(1)
            .map(|at| siblings[at])
            .ok_or(EditError::NoPreviousSibling)?;
        self.refuse_if_linked(previous)?;
        let place = Place {
            parent: Some(previous),
            after: outline.children(previous).last().copied(),
        };
        self.edit(Edit::Reparent {
            task,
            place,
            siblings: Siblings::Keep,
        })?;
        Ok(task)
    }

    /// Outdent the selected row: it moves to just after its parent and adopts the siblings below
    /// it as its last children (MS Project's rule). Recorded for undo, which hands them back;
    /// the selection follows the task. Refused, changing nothing, at the top level, under a row
    /// filter or lookahead window, during a bar press, or when the row has a link and adopts a
    /// sibling (it would become a summary). Returns the task.
    ///
    /// # Errors
    /// An [`EditError`] for each refusal above, else the edit's refusal.
    ///
    /// # Panics
    /// Panics if the session has no outline. Unreachable: the target check refuses that first.
    pub fn outdent_selected(&mut self) -> Result<TaskId, EditError> {
        let task = self.reparent_target()?;
        let outline = self.outline.as_ref().expect("checked");
        let parent = outline.parent(task).ok_or(EditError::AlreadyTopLevel)?;
        let (siblings, at) = siblings_of(outline, task);
        let adopted = siblings.len() - at - 1;
        let place = Place {
            parent: outline.parent(parent),
            after: Some(parent),
        };
        if adopted > 0 {
            self.refuse_if_linked(task)?;
        }
        self.edit(Edit::Reparent {
            task,
            place,
            siblings: Siblings::Adopt(adopted),
        })?;
        Ok(task)
    }

    /// Move the selected row and its subtree above the sibling before it. Recorded for undo; the
    /// selection follows the task. Refused, changing nothing, as indent is, and for the first
    /// of its siblings. Returns the task.
    ///
    /// # Errors
    /// An [`EditError`] for each refusal above, else the edit's refusal.
    pub fn move_selected_up(&mut self) -> Result<TaskId, EditError> {
        self.move_selected(true)
    }

    /// Move the selected row and its subtree below the sibling after it. As
    /// [`Session::move_selected_up`], with the last of its siblings refused.
    ///
    /// # Errors
    /// An [`EditError`] for each refusal above, else the edit's refusal.
    pub fn move_selected_down(&mut self) -> Result<TaskId, EditError> {
        self.move_selected(false)
    }

    /// One `Reparent` under the parent the row has, with `Siblings::Keep`.
    fn move_selected(&mut self, up: bool) -> Result<TaskId, EditError> {
        let task = self.reparent_target()?;
        let outline = self.outline.as_ref().expect("checked");
        let (siblings, at) = siblings_of(outline, task);
        // The row lands after the sibling two above (first when none), or after the one below.
        let after = if up {
            if at == 0 {
                return Err(EditError::NoPreviousSibling);
            }
            at.checked_sub(2).map(|before| siblings[before])
        } else {
            Some(*siblings.get(at + 1).ok_or(EditError::NoNextSibling)?)
        };
        let parent = outline.parent(task);
        self.edit(Edit::Reparent {
            task,
            place: Place { parent, after },
            siblings: Siblings::Keep,
        })?;
        Ok(task)
    }

    /// Refuse a row that has a link and is about to gain a child, naming it (a summary takes no
    /// links, gs#47 ruling 5), so the person sees the row, not the dependency the outline names.
    fn refuse_if_linked(&self, task: TaskId) -> Result<(), EditError> {
        let linked = self
            .schedule
            .live_dependencies()
            .any(|(_, link)| link.pred == task || link.succ == task);
        if !linked {
            return Ok(());
        }
        let name = self.meta[task.0 as usize].name.clone();
        Err(EditError::HasLinks { name })
    }

    /// The selected task, once the session can reparent at all.
    fn reparent_target(&self) -> Result<TaskId, EditError> {
        self.refuse_read_only()?;
        self.check_structural()?;
        if self.filtering() {
            return Err(EditError::ReparentFiltered);
        }
        self.refuse_grouped(EditError::ReparentGrouped)?;
        let task = self.selection.as_ref().ok_or(EditError::NoSelection)?.task;
        Ok(TaskId(task))
    }

    /// Move `task` and its subtree to `place`. A person's reparent (`meta` is `None`) gives the
    /// task the level below the parent `place` names; `meta`, from a history step, puts back the
    /// exact level the task held when the step was recorded. The subtree shifts by the same
    /// delta; adopted siblings and released children shift by one level more or less, so each
    /// keeps its offset from the task and the inverse shifts back exactly. A `host`'s move is taken
    /// while a row filter or a grouping is on and leaves the selection and the scroll.
    #[expect(clippy::too_many_lines, reason = "plans the move before changing it")]
    pub(crate) fn run_reparent(
        &mut self,
        task: TaskId,
        place: Place,
        siblings: Siblings,
        meta: Option<Meta>,
        host: bool,
    ) -> Done {
        self.check_structural()?;
        if !host && self.filtering() {
            return Err(EditError::ReparentFiltered);
        }
        if !host {
            self.refuse_grouped(EditError::ReparentGrouped)?;
        }
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let outline = self.outline.as_ref().expect("checked");
        let order = outline.preorder();
        let start = outline.preorder_position(task);
        let own_end = start + outline.subtree(task).len();
        let (neighbours, at) = siblings_of(outline, task);
        let mut end = own_end;
        let mut release_from = own_end;
        match siblings {
            Siblings::Keep => {}
            Siblings::Adopt(n) => {
                // `n` is a public count: add it checked, so an oversized one is refused, not a panic.
                let adopted = (at + 1)
                    .checked_add(n)
                    .and_then(|to| neighbours.get(at + 1..to))
                    .ok_or(EditError::BadPlace)?;
                if let Some(&last) = adopted.last() {
                    end = outline.preorder_position(last) + outline.subtree(last).len();
                }
            }
            Siblings::Release(n) => {
                let children = outline.children(task);
                let released = children
                    .len()
                    .checked_sub(n)
                    .map(|from| &children[from..])
                    .ok_or(EditError::BadPlace)?;
                if let Some(&first) = released.first() {
                    release_from = outline.preorder_position(first);
                }
            }
        }
        // Where the block lands among the rest: after `after`'s whole subtree, else right under
        // `parent`, else first. A place inside the moving block, or an `after` that is not a
        // child of `parent`, is refused.
        let removed = end - start;
        let inside = |t: TaskId| (start..end).contains(&outline.preorder_position(t));
        let rest_position = |t: TaskId| {
            let position = outline.preorder_position(t);
            position - if position >= end { removed } else { 0 }
        };
        let landing = match (place.after, place.parent) {
            (Some(after), parent)
                if self.is_live(after) && !inside(after) && outline.parent(after) == parent =>
            {
                let position = outline.preorder_position(after);
                let holds_block =
                    position < start && start < position + outline.subtree(after).len();
                let length = outline.subtree(after).len() - if holds_block { removed } else { 0 };
                rest_position(after) + length
            }
            (None, Some(parent)) if self.is_live(parent) && !inside(parent) => {
                rest_position(parent) + 1
            }
            (None, None) => 0,
            _ => return Err(EditError::BadPlace),
        };
        let mut wbs: Vec<TaskId> = order[..start]
            .iter()
            .chain(&order[end..])
            .copied()
            .collect();
        wbs.splice(landing..landing, order[start..end].iter().copied());

        // The new parent and level of every task in the block, worked out before anything changes.
        let level_of = |t: TaskId| i32::from(self.meta[t.0 as usize].outline_level);
        let new_level = match meta {
            Some(meta) => i32::from(meta.level),
            None => place.parent.map_or(1, |parent| level_of(parent) + 1),
        };
        let shift = new_level - level_of(task);
        let mut held: Vec<Held> = Vec::with_capacity(end - start);
        let mut changes: Vec<Held> = Vec::with_capacity(end - start);
        for (offset, &t) in order[start..end].iter().enumerate() {
            let index = t.0 as usize;
            held.push((
                index,
                self.meta[index].parent,
                self.meta[index].outline_level,
            ));
            let position = start + offset;
            let (parent, by) = if position == start {
                (place.parent, shift)
            } else if position >= own_end {
                // An adopted sibling: its own subtree keeps its parents but its roots take the task.
                let root = outline.parent(t) == outline.parent(task);
                (
                    if root {
                        Some(task)
                    } else {
                        self.meta[index].parent
                    },
                    shift + 1,
                )
            } else if position >= release_from {
                let root = outline.parent(t) == Some(task);
                (
                    if root {
                        place.parent
                    } else {
                        self.meta[index].parent
                    },
                    shift - 1,
                )
            } else {
                (self.meta[index].parent, shift)
            };
            let level = u8::try_from(level_of(t) + by).map_err(|_| EditError::BadPlace)?;
            changes.push((index, parent, level));
        }
        let before = Place::of(outline, task);
        let task_held = Meta {
            parent: self.meta[task.0 as usize].parent,
            level: self.meta[task.0 as usize].outline_level,
        };
        let set = |meta: &mut [TaskMeta], rows: &[Held]| {
            for &(index, parent, level) in rows {
                meta[index].parent = parent;
                meta[index].outline_level = level;
            }
        };
        set(&mut self.meta, &changes);
        let elapsed = self.rebuild_structure(&wbs, |s| set(&mut s.meta, &held))?;
        // Selected and shown: `go_to` opens a collapsed new parent.
        if !host {
            self.go_to(task.0);
        }
        self.refresh_selection();
        let inverse = Step {
            edit: Edit::Reparent {
                task,
                place: before,
                siblings: siblings.inverse(),
            },
            meta: Some(task_held),
        };
        Ok((inverse, elapsed))
    }
}

/// The siblings of `task` (its parent's children, or the roots) and its index among them.
fn siblings_of(outline: &crate::outline::Outline, task: TaskId) -> (&[TaskId], usize) {
    let siblings = outline
        .parent(task)
        .map_or(outline.roots(), |parent| outline.children(parent));
    let at = siblings
        .iter()
        .position(|&sibling| sibling == task)
        .expect("a live task is one of its parent's children");
    (siblings, at)
}
