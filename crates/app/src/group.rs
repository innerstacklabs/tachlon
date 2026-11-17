//! Rows grouped by one activity code type (decision 0015): a flat list, one header per value
//! and under it the leaves that carry the value. A header is a row of the view, not a task, so
//! `Timeline::order` stays a list of tasks; only the table rows and the canvas read the view.

use crate::Session;
use crate::edit::EditError;
use crate::meta::TaskMeta;
use crate::outline::Outline;
use engine::TaskId;
use std::collections::{BTreeMap, BTreeSet};
use timeline::HIDDEN_ROW;

/// The leaves under one header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The code value, the header's label; `None` for the leaves with no value of the type,
    /// whose header is last and drawn as an empty-value marker.
    pub value: Option<String>,
    /// The leaves, in outline order.
    pub rows: Vec<TaskId>,
}

/// One row on screen while the list is grouped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewRow {
    /// A leaf task.
    Task(u32),
    /// A group's header: its value (`None` for no value) and how many leaves it holds.
    Header(Option<String>, u32),
}

/// The grouping a session shows: view state, never saved and never in the undo history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grouping {
    /// The activity code type the rows are grouped by.
    pub code_type: String,
    /// The rows on screen, as of the last row rebuild.
    pub rows: Vec<ViewRow>,
    /// The headers (by value) whose leaves are hidden; it ends with the grouping.
    pub collapsed: BTreeSet<Option<String>>,
}

/// The default grouping key: the first code type of [`crate::Session::activity_codes`], which
/// lists the types in order of name. `None` for a file with no activity codes.
#[must_use]
pub fn default_code_type(codes: &[(String, Vec<String>)]) -> Option<&str> {
    codes.first().map(|(code_type, _)| code_type.as_str())
}

/// One [`Group`] per value of `code_type` that a kept leaf carries, in order of value (digit
/// runs read as numbers, as [`crate::Session::activity_codes`] lists them), then the kept leaves
/// with no value. A leaf sits under its first value of the type in its own list of codes; a
/// summary is in no group. `kept` counts the leaves the filters keep among the first `i`
/// preorder rows ([`crate::Session`]'s filter prefix); `None` keeps every leaf.
#[must_use]
pub fn group_by_code(
    outline: &Outline,
    meta: &[TaskMeta],
    code_type: &str,
    kept: Option<&[u32]>,
) -> Vec<Group> {
    let mut by_value: BTreeMap<&str, Vec<TaskId>> = BTreeMap::new();
    let mut bare = Vec::new();
    for (i, &task) in outline.preorder().iter().enumerate() {
        if outline.is_summary(task) || kept.is_some_and(|counts| counts[i + 1] == counts[i]) {
            continue;
        }
        let codes = meta
            .get(task.0 as usize)
            .map(|m| m.activity_codes.as_slice());
        match codes
            .unwrap_or_default()
            .iter()
            .find(|code| code.code_type == code_type)
        {
            Some(code) => by_value.entry(&code.value).or_default().push(task),
            None => bare.push(task),
        }
    }
    let mut groups: Vec<_> = by_value.into_iter().collect();
    // Stable: values the natural order calls equal keep their order by bytes.
    groups.sort_by(|a, b| import::natural(a.0, b.0));
    let named = groups.into_iter().map(|(v, r)| (Some(v.to_owned()), r));
    let bare = (!bare.is_empty()).then_some((None, bare));
    let group = |(value, rows)| Group { value, rows };
    named.chain(bare).map(group).collect()
}

/// The order (the leaves, header by header) and the rows on screen for `groups`; a header in
/// `collapsed` keeps its count and shows none of its leaves.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a group holds at most one entry per task, and a task id is a u32"
)]
pub(crate) fn view_rows(
    groups: Vec<Group>,
    collapsed: &BTreeSet<Option<String>>,
) -> (Vec<u32>, Vec<ViewRow>) {
    let (mut order, mut rows) = (Vec::new(), Vec::new());
    for group in groups {
        let count = group.rows.len() as u32;
        let open = !collapsed.contains(&group.value);
        rows.push(ViewRow::Header(group.value, count));
        if open {
            order.extend(group.rows.iter().map(|task| task.0));
            rows.extend(group.rows.iter().map(|task| ViewRow::Task(task.0)));
        }
    }
    (order, rows)
}

impl Session {
    /// The activity code type the rows are grouped by, if any.
    pub fn group_by(&self) -> Option<&str> {
        self.group.as_ref().map(|group| group.code_type.as_str())
    }

    /// Group the rows by the activity code type `code_type` (decision 0015), or show the outline
    /// again with `None`. It is view state: not saved and not an undo step; collapse comes back
    /// as it was. Returns false, and changes nothing, when there is no outline or a bar press is
    /// active.
    pub fn set_group_by(&mut self, code_type: Option<&str>) -> bool {
        if self.outline.is_none() || self.busy() {
            return false;
        }
        if self.group_by() == code_type {
            return true;
        }
        self.group = code_type.map(|code_type| Grouping {
            code_type: code_type.to_owned(),
            rows: Vec::new(),
            collapsed: BTreeSet::new(),
        });
        if !self.apply_rows() {
            self.stamp = self.stamp().wrapping_add(1);
        }
        true
    }

    /// Hide the leaves under the group header `value` (`None` is the empty-value header), or show
    /// them again. View state like the grouping: not saved, not an undo step, and the outline's
    /// collapse is untouched. Returns false, and changes nothing, when the rows are not grouped,
    /// no header has that value, or a bar press is active.
    pub fn toggle_group(&mut self, value: Option<&str>) -> bool {
        let Some(group) = self.group.as_ref().filter(|_| !self.busy()) else {
            return false;
        };
        let named = |row: &ViewRow| matches!(row, ViewRow::Header(v, _) if v.as_deref() == value);
        if !group.rows.iter().any(named) {
            return false;
        }
        let value = value.map(str::to_owned);
        if let Some(group) = self.group.as_mut()
            && !group.collapsed.remove(&value)
        {
            group.collapsed.insert(value);
        }
        self.apply_rows();
        true
    }

    /// The header on screen row `row` while grouped, with whether its group is collapsed; `None`
    /// on a task row or past the rows.
    pub(crate) fn header(&self, row: usize) -> Option<(Option<&str>, u32, bool)> {
        let group = self.group.as_ref()?;
        match group.rows.get(row)? {
            ViewRow::Header(value, count) => {
                Some((value.as_deref(), *count, group.collapsed.contains(value)))
            }
            ViewRow::Task(_) => None,
        }
    }

    /// Alt+Left (`open: false`) or Alt+Right while grouped: close or open the group of the
    /// selected task, which stays selected under a closed header. False when nothing changes.
    pub(crate) fn toggle_selected_group(&mut self, open: bool) -> bool {
        let Some(task) = self.selection().map(|selection| selection.task) else {
            return false;
        };
        let Some(group) = self.group.as_ref() else {
            return false;
        };
        let value = self.group_value(group, task);
        let closed = group.collapsed.contains(&value);
        closed == open && self.toggle_group(value.as_deref())
    }

    /// The screen row of the header of `task`'s group while grouped: where the cursor stands when
    /// the group is closed and the task has no row.
    pub(crate) fn group_header_row(&self, task: u32) -> Option<usize> {
        let group = self.group.as_ref()?;
        let value = self.group_value(group, task);
        let header = |row: &ViewRow| matches!(row, ViewRow::Header(v, _) if *v == value);
        group.rows.iter().position(header)
    }

    /// The value of the header `task` sits under: its first code of the grouping type, or `None`
    /// for no value.
    fn group_value(&self, group: &Grouping, task: u32) -> Option<String> {
        let codes = &self.meta().get(task as usize)?.activity_codes;
        let code = codes.iter().find(|code| code.code_type == group.code_type);
        code.map(|code| code.value.clone())
    }

    /// A refusal while grouped: in a flat list the row above is not the outline sibling.
    pub(crate) fn refuse_grouped(&self, error: EditError) -> Result<(), EditError> {
        self.group.as_ref().map_or(Ok(()), |_| Err(error))
    }

    /// Put `order` on the timeline, with the grouped view over it while grouped.
    pub(crate) fn show_rows(&mut self, order: Vec<u32>) {
        self.timeline.set_order(order);
        if let Some(view) = self.screen_rows() {
            self.timeline.set_view(view);
        }
    }

    /// Whether the timeline's rows on screen differ from the grouping's.
    pub(crate) fn view_stale(&self) -> bool {
        let view = self.screen_rows();
        self.timeline.rows() != view.as_deref().unwrap_or(self.timeline.order())
    }

    fn screen_rows(&self) -> Option<Vec<u32>> {
        let rows = &self.group.as_ref()?.rows;
        let task = |row: &ViewRow| match row {
            ViewRow::Task(task) => *task,
            ViewRow::Header(..) => HIDDEN_ROW,
        };
        Some(rows.iter().map(task).collect())
    }
}
