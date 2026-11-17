//! Row filters and the visible row order: [`RowFilter`], the lookahead window, collapse
//! and the rows they leave (gs#243, split out of `lib.rs`).

use crate::outline::Outline;
use crate::{RowFilter, Session, lookahead};
use engine::TaskId;
use import::ActivityCode;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

impl Session {
    /// Whether rows without a critical leaf in their subtree are hidden.
    pub fn critical_only(&self) -> bool {
        self.row_filter == RowFilter::Critical
    }

    /// Turn the critical-only row filter on or off. Returns false when there
    /// is no outline or a bar press is active.
    pub fn set_critical_only(&mut self, on: bool) -> bool {
        let wanted = if on {
            RowFilter::Critical
        } else if self.row_filter == RowFilter::Critical {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// Whether rows without a longest-path leaf in their subtree are hidden.
    pub fn longest_path(&self) -> bool {
        self.row_filter == RowFilter::LongestPath
    }

    /// Turn the longest-path row filter on or off. Returns false when there
    /// is no outline or a bar press is active.
    pub fn set_longest_path(&mut self, on: bool) -> bool {
        let wanted = if on {
            RowFilter::LongestPath
        } else if self.row_filter == RowFilter::LongestPath {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// Whether rows without a leaf that finishes later than the baseline are hidden.
    pub fn slipped_only(&self) -> bool {
        self.row_filter == RowFilter::Slipped
    }

    /// Turn the slipped row filter on or off. Returns false when there is no outline, no
    /// baseline (to turn it on), or a bar press is active.
    pub fn set_slipped_only(&mut self, on: bool) -> bool {
        let wanted = if on {
            if self.baseline.is_none() {
                return false;
            }
            RowFilter::Slipped
        } else if self.row_filter == RowFilter::Slipped {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// The activity code the row filter keeps, when that filter is on.
    pub fn activity_code_filter(&self) -> Option<&ActivityCode> {
        match &self.row_filter {
            RowFilter::ActivityCode(code) => Some(code),
            _ => None,
        }
    }

    /// Keep only the leaves that carry the activity code `code_type` = `value`, and the summaries
    /// that contain one. It replaces any other row filter. Returns false, and changes nothing,
    /// when no live task carries that code, there is no outline, or a bar press is active.
    pub fn set_activity_code_filter(&mut self, code_type: &str, value: &str) -> bool {
        let wanted = ActivityCode {
            code_type: code_type.to_owned(),
            value: value.to_owned(),
        };
        if self.live_tasks_with(&wanted).next().is_none() {
            return false;
        }
        self.set_row_filter(RowFilter::ActivityCode(Arc::new(wanted)))
    }

    /// Turn the activity code filter off. Another row filter stays on. Returns false when there
    /// is no outline or a bar press is active.
    pub fn clear_activity_code_filter(&mut self) -> bool {
        let wanted = match &self.row_filter {
            RowFilter::ActivityCode(_) => RowFilter::Off,
            other => other.clone(),
        };
        self.set_row_filter(wanted)
    }

    /// The activity code types the live tasks carry, each with its values: both in order of name,
    /// and every value once. Empty for a file with no activity codes.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    pub fn activity_codes(&self) -> Vec<(String, Vec<String>)> {
        let mut found: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for (index, meta) in self.meta.iter().enumerate() {
            if self.schedule.is_deleted(TaskId(index as u32)) {
                continue;
            }
            for code in &meta.activity_codes {
                found
                    .entry(&code.code_type)
                    .or_default()
                    .insert(&code.value);
            }
        }
        found
            .into_iter()
            .map(|(code_type, values)| {
                // The set drops repeats; the sort reads digit runs as numbers, so `L2` comes before `L10`.
                let mut values: Vec<String> = values.into_iter().map(str::to_owned).collect();
                values.sort_by(|a, b| import::natural(a, b));
                (code_type.to_owned(), values)
            })
            .collect()
    }

    /// The ids of the live tasks that carry `code`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    fn live_tasks_with<'a>(&'a self, code: &'a ActivityCode) -> impl Iterator<Item = u32> + 'a {
        self.meta
            .iter()
            .enumerate()
            .filter(move |(index, meta)| {
                !self.schedule.is_deleted(TaskId(*index as u32))
                    && meta.activity_codes.contains(code)
            })
            .map(|(index, _)| index as u32)
    }

    /// The active row filter.
    pub fn row_filter(&self) -> RowFilter {
        self.row_filter.clone()
    }

    /// The lookahead window, if one is set.
    pub fn lookahead(&self) -> Option<lookahead::Lookahead> {
        self.lookahead
    }

    /// Where a lookahead window starts when the reader has not picked a date: the file's data
    /// date, or the project origin when it has none.
    pub fn default_lookahead_start(&self) -> engine::Instant {
        self.data_date.unwrap_or(0)
    }

    /// Keep only the tasks whose early dates intersect the window of `weeks` weeks from `start`
    /// (see [`lookahead`]), besides the row filter and collapse. Returns false, and changes
    /// nothing, when `weeks` is 0 or above [`lookahead::MAX_WEEKS`], there is no outline, or a
    /// bar press is active.
    pub fn set_lookahead(&mut self, start: engine::Instant, weeks: u32) -> bool {
        match lookahead::Lookahead::new(start, weeks) {
            Some(window) => self.change_lookahead(Some(window)),
            None => false,
        }
    }

    /// Remove the lookahead window. Returns false when there is no outline or a bar press is
    /// active.
    pub fn clear_lookahead(&mut self) -> bool {
        self.change_lookahead(None)
    }

    fn change_lookahead(&mut self, wanted: Option<lookahead::Lookahead>) -> bool {
        if self.outline.is_none() || self.busy() {
            return false;
        }
        if self.lookahead == wanted {
            return true;
        }
        self.lookahead = wanted;
        if !self.apply_rows() {
            self.stamp = self.stamp().wrapping_add(1);
        }
        true
    }

    /// A row filter or a lookahead window limits the visible rows.
    pub(crate) fn filtering(&self) -> bool {
        self.row_filter != RowFilter::Off || self.lookahead.is_some()
    }

    pub(crate) fn set_row_filter(&mut self, wanted: RowFilter) -> bool {
        if self.outline.is_none() || self.busy() {
            return false;
        }
        if self.row_filter == wanted {
            return true;
        }
        self.row_filter = wanted;
        if !self.apply_rows() {
            self.stamp = self.stamp().wrapping_add(1);
        }
        true
    }

    /// Flip a summary's collapsed state and rebuild the visible row order.
    pub fn toggle(&mut self, task: TaskId) -> bool {
        if self.busy() {
            return false;
        }
        let index = task.0 as usize;
        let Some(outline) = self.outline.as_ref() else {
            return false;
        };
        if index >= outline.len() || !outline.is_summary(task) {
            return false;
        }

        self.collapsed[index] = !self.collapsed[index];
        let order = self.visible_rows();
        let place = self.place();
        self.show_rows(order);
        self.keep_place(place);
        self.rebuild_links();
        self.sync_selection_row();
        self.stamp = self.stamp().wrapping_add(1);
        true
    }

    /// Collapse `tasks` as the session opens, as a document that holds some is read. It is not an
    /// edit: it leaves no undo step. The flag is kept on a task that has no children now (a
    /// reparent leaves it on a summary it emptied, and a save writes it); a task past the outline
    /// is skipped, and a session with no outline collapses nothing.
    pub fn with_collapsed(mut self, tasks: &[TaskId]) -> Session {
        let Some(outline) = self.outline.as_ref() else {
            return self;
        };
        for &task in tasks {
            if (task.0 as usize) < outline.len() {
                self.collapsed[task.0 as usize] = true;
            }
        }
        let order = self.visible_rows();
        self.show_rows(order);
        self.rebuild_links();
        self.sync_selection_row();
        self.stamp = self.stamp().wrapping_add(1);
        self
    }

    /// Visible outline rows after collapse and active row filtering, or while grouped the kept
    /// leaves header by header (outline collapse hides none; a collapsed group hides its own).
    fn visible_rows(&mut self) -> Vec<u32> {
        let Some(outline) = self.outline.as_ref() else {
            return self.timeline().order().to_vec();
        };
        let preorder = outline.preorder();
        let kept_before = self.filter_leaf_prefix(outline);
        self.kept_leaves = kept_before
            .as_ref()
            .and_then(|counts| counts.last().copied());
        self.path_line = self.path_line(outline);
        if let Some(group) = self.group.as_mut() {
            let kept = kept_before.as_deref();
            let groups = crate::group::group_by_code(outline, &self.meta, &group.code_type, kept);
            let (order, rows) = crate::group::view_rows(groups, &group.collapsed);
            group.rows = rows;
            return order;
        }
        let mut rows = Vec::with_capacity(preorder.len());
        let mut i = 0;
        while i < preorder.len() {
            let task = preorder[i];
            let len = outline.subtree(task).len();
            if let Some(counts) = &kept_before
                && counts[i + len] == counts[i]
            {
                i += len;
                continue;
            }
            rows.push(task.0);
            i += if self.collapsed[task.0 as usize] {
                len
            } else {
                1
            };
        }
        rows
    }

    /// Counts kept leaves among the first `i` preorder rows. A task is kept
    /// by the active filters (the row filter and the lookahead window, both
    /// when both are set) when the count changes across its subtree's
    /// preorder slice. There is no prefix while filtering is off.
    pub(crate) fn filter_leaf_prefix(&self, outline: &Outline) -> Option<Vec<u32>> {
        if !self.filtering() {
            return None;
        }
        let longest = (self.row_filter == RowFilter::LongestPath).then(|| {
            let mut members = vec![false; self.schedule.tasks().len()];
            for task in self.schedule.longest_path(&self.solution) {
                members[task.0 as usize] = true;
            }
            members
        });
        let reported = self.moved_members();
        let checked = self.check_members();
        let joined =
            (self.row_filter == RowFilter::JoinedCritical).then(|| self.path_members(true));
        let left = (self.row_filter == RowFilter::LeftCritical).then(|| self.path_members(false));
        let mut counts = Vec::with_capacity(outline.len() + 1);
        counts.push(0);
        for &task in outline.preorder() {
            let dates = &self.solution.dates[task.0 as usize];
            let kept = !outline.is_summary(task)
                && match &self.row_filter {
                    RowFilter::Off => true,
                    RowFilter::Critical => dates.is_critical(),
                    RowFilter::NearCritical => self.near_critical_task(task),
                    RowFilter::FloatLost => self.float_lost_task(task),
                    RowFilter::LongestPath => longest
                        .as_ref()
                        .is_some_and(|members| members[task.0 as usize]),
                    RowFilter::Moved => reported
                        .as_ref()
                        .is_some_and(|members| members[task.0 as usize]),
                    // A task the baseline has no entry for (added since) is not kept.
                    RowFilter::Slipped => self
                        .baseline
                        .as_ref()
                        .and_then(|baseline| baseline.task(task))
                        .is_some_and(|then| dates.ef > then.ef),
                    RowFilter::Status(kind) => kind.holds(&self.schedule.tasks()[task.0 as usize]),
                    RowFilter::Check(_) => checked
                        .as_ref()
                        .is_some_and(|members| members[task.0 as usize]),
                    RowFilter::StartConstraint => self.start_constraint(task).is_some(),
                    RowFilter::Resource(name) => {
                        self.resources(task).iter().any(|r| r.name == **name)
                    }
                    RowFilter::JoinedCritical => joined
                        .as_ref()
                        .is_some_and(|members| members[task.0 as usize]),
                    RowFilter::LeftCritical => left
                        .as_ref()
                        .is_some_and(|members| members[task.0 as usize]),
                    RowFilter::ActivityCode(code) => self
                        .meta
                        .get(task.0 as usize)
                        .is_some_and(|meta| meta.activity_codes.contains(code)),
                }
                && self
                    .lookahead
                    .is_none_or(|window| window.intersects(dates.es, dates.ef));
            counts.push(counts[counts.len() - 1] + u32::from(kept));
        }
        Some(counts)
    }

    /// Install the visible row order and rebuild row-indexed state when it
    /// changed (bumping `stamp`), then mark any deferred refresh complete.
    /// Returns whether the rows changed.
    pub(crate) fn apply_rows(&mut self) -> bool {
        self.end_float_lost_without_float();
        let order = self.visible_rows();
        let changed = order != self.timeline().order() || self.view_stale();
        if changed {
            let place = self.place();
            self.show_rows(order);
            self.keep_place(place);
            self.rebuild_links();
            self.sync_selection_row();
            self.stamp = self.stamp().wrapping_add(1);
        }
        self.rows_stale = false;
        changed
    }

    /// Refresh filtered rows after a solution change, deferring while a
    /// press is active so rows do not shift beneath the pointer.
    pub(crate) fn follow_filter(&mut self) {
        if !self.filtering() {
            return;
        }
        if self.busy() {
            self.rows_stale = true;
            return;
        }
        self.apply_rows();
    }
}
