//! Range selection (gs#423, story S2 of #300): the selected task is the range's anchor, and the
//! extent is the other end. The rows between them are the table's visible rows, so a collapsed
//! summary's children and the rows a filter hides are never in a range, and a collapse or a
//! filter needs no special case. A range lasts while its anchor is the selection: selecting a
//! task (a click, an arrow, a search hit) or clearing the selection ends it.

use crate::Session;
use engine::TaskId;

/// The two ends of a range, as tasks. Rows are looked up when read, never stored, so a collapse
/// or a filter moves the rows under a range without touching it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Range {
    anchor: u32,
    extent: u32,
}

/// A range in table rows: `first` to `last` inclusive, and the row of the extent (the end that
/// moves), which is one of the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeRows {
    pub first: u32,
    pub last: u32,
    pub extent: u32,
}

impl Session {
    /// Move the range's extent `delta` visible rows (negative: up), stopping at the first or last
    /// row; the anchor stays. With no range the extent starts at the anchor, and with nothing
    /// selected the first row is selected and is the range. Returns the extent task, or `None`,
    /// changing nothing, while a bar press or a drag is in progress or when there are no rows.
    pub fn extend_selection(&mut self, delta: i32) -> Option<u32> {
        if self.busy() {
            return None;
        }
        let first = *self.timeline().order().first()?;
        if self.selection().is_none() {
            self.select(first);
            self.range = Some(Range {
                anchor: first,
                extent: first,
            });
            return Some(first);
        }
        let anchor = self.selection()?.task;
        let from = self.visible_row_of(self.valid_range().map_or(anchor, |range| range.extent))?;
        let to = self.timeline().step_task_row(Some(from), delta)?;
        let extent = self.timeline().task_at_row(to)?;
        self.range = Some(Range { anchor, extent });
        Some(extent)
    }

    /// Make the selected row a range, whole (`Shift+Space`): a range of that one row, kept as it is
    /// when there is a range already. False, changing nothing, with nothing selected, with the
    /// selection on no row, or while a bar press or a drag is in progress.
    pub fn select_row_range(&mut self) -> bool {
        let Some(anchor) = self.selection().map(|selected| selected.task) else {
            return false;
        };
        if self.busy() || self.visible_row_of(anchor).is_none() {
            return false;
        }
        if self.valid_range().is_none() {
            self.range = Some(Range {
                anchor,
                extent: anchor,
            });
        }
        true
    }

    /// Make every visible row a range (`Ctrl+A`): the first row is selected and is the anchor, the
    /// last row is the extent, and any range there was is replaced. False, changing nothing, while
    /// a bar press or a drag is in progress or when there are no rows.
    pub fn select_all_rows(&mut self) -> bool {
        if self.busy() {
            return false;
        }
        let (Some(&first), Some(&last)) = (
            self.timeline().order().first(),
            self.timeline().order().last(),
        ) else {
            return false;
        };
        self.select(first);
        self.range = Some(Range {
            anchor: first,
            extent: last,
        });
        true
    }

    /// Extend the range to `task` (`Shift`+click on its row); with nothing selected, select `task`
    /// as a range of one row. False, changing nothing, while a bar press or a drag is in progress,
    /// and for a task that is deleted, unknown or has no row.
    pub fn extend_to(&mut self, task: u32) -> bool {
        if self.busy() || !self.is_live(TaskId(task)) || self.timeline().row_of_task(task).is_none()
        {
            return false;
        }
        if self.selection().is_none() {
            self.select(task);
        }
        let Some(anchor) = self.selection().map(|selected| selected.task) else {
            return false;
        };
        self.range = Some(Range {
            anchor,
            extent: task,
        });
        true
    }

    /// The range's anchor and extent tasks, or `None` with no range. A range whose anchor is no
    /// longer the selection, whose end is deleted, or whose end has no row in the table (no
    /// visible ancestor either) is not a range.
    pub fn range(&self) -> Option<(u32, u32)> {
        self.valid_range().map(|range| (range.anchor, range.extent))
    }

    /// The range as table rows. A collapsed end stands on the row of its nearest visible ancestor.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row indexes the timeline order, which holds at most one entry per task, so it is below the slot count; Schedule::add_task casts that count to u32 with no check against u32::MAX, so past 2^32 slots it wraps"
    )]
    pub fn range_rows(&self) -> Option<RangeRows> {
        let range = self.valid_range()?;
        let anchor = self.visible_row_of(range.anchor)? as u32;
        let extent = self.visible_row_of(range.extent)? as u32;
        Some(RangeRows {
            first: anchor.min(extent),
            last: anchor.max(extent),
            extent,
        })
    }

    /// The tasks on the range's rows, in table order. Only visible rows count: the children of a
    /// collapsed summary inside the range, and the rows a filter hides, are not here.
    pub fn range_tasks(&self) -> Vec<u32> {
        self.range_rows().map_or_else(Vec::new, |rows| {
            (rows.first..=rows.last)
                .filter_map(|row| self.timeline().task_at_row(row as usize))
                .collect()
        })
    }

    /// Drop a range that is no longer one. Called with each frame, so a task that comes back (a
    /// redo) does not bring a dropped range back with it.
    pub(crate) fn prune_range(&mut self) {
        if self.range.is_some() && self.valid_range().is_none() {
            self.range = None;
        }
    }

    /// The `range` member of the status JSON: `[first, last, extent]` rows, or `null`.
    pub(crate) fn range_json(&self) -> String {
        self.range_rows().map_or_else(
            || "null".to_string(),
            |rows| format!("[{},{},{}]", rows.first, rows.last, rows.extent),
        )
    }

    fn valid_range(&self) -> Option<Range> {
        let range = self.range?;
        let usable = |task: u32| self.visible_row_of(task).is_some();
        (self.selection()?.task == range.anchor && usable(range.anchor) && usable(range.extent))
            .then_some(range)
    }
}
