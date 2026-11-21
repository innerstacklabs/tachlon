//! Where a task sits in the outline, and where a new one goes: below a person's selection, or where a host's add
//! names it.

use crate::Session;
use crate::outline::Outline;
use engine::TaskId;

/// Where a task sits in the outline: under `parent` (`None`: a root), after sibling `after`
/// (`None`: first child).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub parent: Option<TaskId>,
    pub after: Option<TaskId>,
}

impl Place {
    /// The place `task` has in `outline`.
    pub(crate) fn of(outline: &Outline, task: TaskId) -> Place {
        let parent = outline.parent(task);
        let siblings = parent.map_or(outline.roots(), |parent| outline.children(parent));
        let at = siblings.iter().position(|&sibling| sibling == task);
        Place {
            parent,
            after: at.and_then(|at| at.checked_sub(1)).map(|at| siblings[at]),
        }
    }
}

impl Session {
    /// Where a task inserted below `selected` goes: a sibling after its whole subtree, at its
    /// level. With none, the last top-level row. A session with no outline has no place.
    pub fn place_below(&self, selected: Option<TaskId>) -> Place {
        let Some(outline) = self.outline.as_ref() else {
            return Place {
                parent: None,
                after: None,
            };
        };
        match selected.filter(|&task| self.is_live(task)) {
            Some(task) => Place {
                parent: outline.parent(task),
                after: Some(task),
            },
            None => Place {
                parent: None,
                after: outline.roots().last().copied(),
            },
        }
    }

    /// Where a host's add goes: under `parent` (none: the parent of `after`, else the top level), after `after`
    /// (none: the last row under that parent).
    pub(crate) fn host_place(&self, parent: Option<TaskId>, after: Option<TaskId>) -> Place {
        let Some(outline) = self.outline.as_ref() else {
            return Place { parent, after };
        };
        let parent = parent.or_else(|| after.and_then(|after| outline.parent(after)));
        let siblings = parent.map_or(outline.roots(), |parent| outline.children(parent));
        let after = after.or_else(|| siblings.last().copied());
        Place { parent, after }
    }
}
