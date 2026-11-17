//! Task-name matching and navigation to a found task.

use crate::{Session, meta, meta::TaskMeta, outline::Outline};
use engine::TaskId;

/// Returns task ids whose displayed names, activity IDs or WBS codes contain `query`: a task whose
/// activity ID is exactly `query` (case-insensitively) first, then the rest, each group in outline
/// order when an outline is available and task-id order otherwise.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
)]
#[must_use]
pub fn matches(meta: &[TaskMeta], outline: Option<&Outline>, query: &str) -> Vec<u32> {
    if query.is_empty() {
        return Vec::new();
    }

    let needle = query.to_lowercase();
    let mut found: Vec<u32> = match outline {
        Some(outline) => outline
            .preorder()
            .iter()
            .filter_map(|task| {
                meta.get(task.0 as usize)
                    .filter(|_| is_match(meta, task.0 as usize, &needle))
                    .map(|_| task.0)
            })
            .collect(),
        None => meta
            .iter()
            .enumerate()
            .filter(|&(index, _)| is_match(meta, index, &needle))
            .map(|(task, _)| task as u32)
            .collect(),
    };
    // A stable sort: the exact ID moves up, everything else keeps its order.
    found.sort_by_key(|&task| !is_exact_id(meta, task as usize, &needle));
    found
}

/// Whether the activity ID of task `index` is `needle` (lowercase), ignoring case.
fn is_exact_id(meta: &[TaskMeta], index: usize, needle: &str) -> bool {
    meta[index]
        .code
        .as_deref()
        .is_some_and(|code| code.to_lowercase() == needle)
}

/// Reads searchable task text: the outline path (the name behind its ancestors' names, so "Level 3"
/// finds its children), the activity ID (`TaskMeta::code`) and the WBS code. `needle` is lowercase.
fn is_match(meta: &[TaskMeta], index: usize, needle: &str) -> bool {
    meta::outline_path(meta, index)
        .to_lowercase()
        .contains(needle)
        || meta[index]
            .code
            .as_deref()
            .is_some_and(|code| code.to_lowercase().contains(needle))
        || meta[index].wbs_code_text().to_lowercase().contains(needle)
}

impl Session {
    /// Finds tasks whose displayed names, activity IDs or WBS codes contain `query`, excluding tasks
    /// the active row filter or lookahead window hides. Collapse does not affect the results. The session keeps them for [`Session::find_go`].
    pub fn find(&mut self, query: &str) -> Vec<u32> {
        self.found = self.find_hits(query);
        self.found.clone()
    }

    /// Select and reveal the `k`th result of the most recent [`Session::find`], and return its
    /// task. `None` past the results, or when [`Session::go_to`] refuses.
    pub fn find_go(&mut self, k: u32) -> Option<u32> {
        let task = *self.found.get(k as usize)?;
        self.go_to(task).then_some(task)
    }

    /// The tasks the most recent [`Session::find`] kept, without searching again, and
    /// [`Session::stamp`] now. Read straight after a find, that is the stamp they were computed
    /// at; while it holds, an edit, an insert or a filter has not changed them since.
    pub fn found(&self) -> (&[u32], u32) {
        (&self.found, self.stamp())
    }

    fn find_hits(&self, query: &str) -> Vec<u32> {
        let found = matches(&self.meta, self.outline(), query);
        if !self.filtering() {
            return found;
        }

        let Some(outline) = self.outline() else {
            return found;
        };
        let Some(kept_before) = self.filter_leaf_prefix(outline) else {
            return found;
        };
        found
            .into_iter()
            .filter(|&task| {
                let task = TaskId(task);
                let start = outline.preorder_position(task);
                kept_before[start + outline.subtree(task).len()] > kept_before[start]
            })
            .collect()
    }

    /// Selects `task`, expanding collapsed ancestors and revealing it in the viewport.
    ///
    /// # Panics
    /// Panics if `task` has no bar start. Unreachable: the guard at the top returns false for a
    /// task with no dates.
    pub fn go_to(&mut self, task: u32) -> bool {
        if self.busy() || self.solution().dates.get(task as usize).is_none() {
            return false;
        }

        if self.filtering() {
            let Some(outline) = self.outline() else {
                return false;
            };
            let task = TaskId(task);
            let Some(kept_before) = self.filter_leaf_prefix(outline) else {
                return false;
            };
            let start = outline.preorder_position(task);
            if kept_before[start + outline.subtree(task).len()] == kept_before[start] {
                return false;
            }
        }

        let from = self.pose();
        let mut collapsed_ancestors = Vec::new();
        if let Some(outline) = self.outline() {
            let mut ancestor = outline.parent(TaskId(task));
            while let Some(parent) = ancestor {
                if self.is_collapsed(parent) {
                    collapsed_ancestors.push(parent);
                }
                ancestor = outline.parent(parent);
            }
        }
        for ancestor in collapsed_ancestors.into_iter().rev() {
            self.toggle(ancestor);
        }

        self.select(task);
        let Some(row) = self.timeline().row_of_task(task) else {
            return false;
        };

        let visible_rows = self.timeline().viewport.visible_rows();
        let visible_row_count = visible_rows.len();
        if !visible_rows.contains(&row) {
            let scrolled = self.timeline.viewport.scroll_y;
            self.timeline
                .viewport
                .scroll_to_row(row.saturating_sub(visible_row_count / 2));
            // A reveal that moves the rows ends the opening fit, as a wheel or a table scroll does.
            #[expect(clippy::float_cmp, reason = "equal exactly when no scroll happened")]
            let stayed = self.timeline.viewport.scroll_y == scrolled;
            self.fit_pending &= stayed;
        }

        let start = self.bar_start(task).expect("the task has dates");
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        let x = self.timeline().viewport.x_of(start as f64);
        if !(0.0..=self.timeline().viewport.width).contains(&x) {
            let width = self.timeline().viewport.width;
            self.fit_pending = false;
            self.timeline.viewport.pan(width * 0.2 - x, 0.0);
        }
        // The reveal flew from where the view was, unless reduced motion asked for the jump.
        self.fly_from(from);
        true
    }

    /// Pans so 00:00 on `day`, written `YYYY-MM-DD`, sits a fifth of the way in from the left (the
    /// pan [`Session::go_to`] uses; the zoom stays) and scrolls so the first row, in table order,
    /// whose bar starts on or after that day is the top row, as far as the scroll range allows. A
    /// day after every bar pans and leaves the rows where they are. False, and nothing changes,
    /// when `day` is not a date or a bar press or drag is in progress.
    pub fn go_to_date(&mut self, day: &str) -> bool {
        if self.busy() {
            return false;
        }
        let Some(at) = self.parse_lookahead_day(day) else {
            return false;
        };
        let first = self
            .timeline()
            .rows()
            .iter()
            .position(|&task| self.bar_start(task).is_some_and(|start| start >= at));
        let width = self.timeline().viewport.width;
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        let x = self.timeline().viewport.x_of(at as f64);
        self.fit_pending = false;
        self.timeline.viewport.pan(width * 0.2 - x, 0.0);
        if let Some(row) = first {
            self.timeline.viewport.scroll_to_row(row);
        }
        true
    }

    /// Where `task`'s bar starts: a summary's span, else the task's early start. `None` for an
    /// unknown task.
    fn bar_start(&self, task: u32) -> Option<engine::Instant> {
        self.timeline()
            .summaries()
            .get(task)
            .map(|span| span.0)
            .or_else(|| {
                self.solution()
                    .dates
                    .get(task as usize)
                    .map(|dates| dates.es)
            })
    }
}
