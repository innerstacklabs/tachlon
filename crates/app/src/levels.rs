//! Commands for collapsing the task outline by level or selected row.

use crate::Session;
use engine::TaskId;

impl Session {
    /// Collapses summaries at `level` and below while expanding shallower summaries.
    pub fn collapse_to_level(&mut self, level: u32) -> bool {
        let Some(outline) = self.outline.as_ref() else {
            return false;
        };
        if level == 0 || self.busy() {
            return false;
        }

        let mut flags_changed = false;
        for (index, collapsed) in self.collapsed.iter_mut().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let task = TaskId(index as u32);
            let wanted = outline.is_summary(task) && outline.depth(task) >= level;
            flags_changed |= *collapsed != wanted;
            *collapsed = wanted;
        }
        self.finish_level_change(flags_changed);
        true
    }

    /// Expands every summary in the outline.
    pub fn expand_all(&mut self) -> bool {
        if self.outline.is_none() || self.busy() {
            return false;
        }

        let flags_changed = self.collapsed.iter().any(|&collapsed| collapsed);
        self.collapsed.fill(false);
        self.finish_level_change(flags_changed);
        true
    }

    /// Collapses the selected summary, or selects the parent of a selected leaf. While grouped it
    /// closes the selected task's group.
    pub fn collapse_selected(&mut self) -> bool {
        if self.group.is_some() {
            return self.toggle_selected_group(false);
        }
        let Some(task) = self
            .selection
            .as_ref()
            .map(|selection| TaskId(selection.task))
        else {
            return false;
        };
        let Some(outline) = self.outline.as_ref() else {
            return false;
        };
        let is_summary = outline.is_summary(task);
        let parent = outline.parent(task);

        if is_summary {
            !self.is_collapsed(task) && self.toggle(task)
        } else if let Some(parent) = parent {
            self.go_to(parent.0)
        } else {
            false
        }
    }

    /// Expands the selected summary when it is collapsed. While grouped it opens the selected
    /// task's group.
    pub fn expand_selected(&mut self) -> bool {
        if self.group.is_some() {
            return self.toggle_selected_group(true);
        }
        let Some(task) = self
            .selection
            .as_ref()
            .map(|selection| TaskId(selection.task))
        else {
            return false;
        };
        let Some(outline) = self.outline.as_ref() else {
            return false;
        };
        outline.is_summary(task) && self.is_collapsed(task) && self.toggle(task)
    }

    /// Expands the selected summary and every summary below it. False, changing nothing, for a
    /// selected leaf, with nothing selected, while busy, or when the whole branch is already open.
    pub fn expand_selected_branch(&mut self) -> bool {
        let Some(task) = self
            .selection
            .as_ref()
            .map(|selection| TaskId(selection.task))
        else {
            return false;
        };
        let Some(outline) = self.outline.as_ref() else {
            return false;
        };
        if self.busy() || !outline.is_summary(task) {
            return false;
        }

        let mut flags_changed = false;
        for below in outline.subtree(task) {
            flags_changed |= std::mem::take(&mut self.collapsed[below.0 as usize]);
        }
        if flags_changed {
            self.finish_level_change(true);
        }
        flags_changed
    }

    fn finish_level_change(&mut self, flags_changed: bool) {
        if !self.apply_rows() && flags_changed {
            self.stamp = self.stamp().wrapping_add(1);
        }
        self.select_visible_ancestor();
    }

    fn select_visible_ancestor(&mut self) {
        let Some(selected) = self.selection.as_ref().map(|selection| selection.task) else {
            return;
        };
        if self.timeline().row_of_task(selected).is_some() {
            return;
        }
        let visible_ancestor = self.outline.as_ref().and_then(|outline| {
            let mut ancestor = outline.parent(TaskId(selected));
            while let Some(task) = ancestor {
                if self.timeline().row_of_task(task.0).is_some() {
                    return Some(task);
                }
                ancestor = outline.parent(task);
            }
            None
        });
        if let Some(ancestor) = visible_ancestor {
            self.select(ancestor.0);
        }
    }
}
