use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use xer::{ProjectRow, TaskRow, WbsRow};

use crate::ImportError;
use crate::notes::{Notes, Section};
use crate::xer_activity_order;
use crate::xer_wbs_code::{self, Codes};

/// The source represented by one entry in a project's preorder outline.
pub(crate) enum PlanKind<'a> {
    /// A real WBS node or the synthetic project root.
    Summary { uid: u32, name: String },
    /// A P6 activity.
    Activity(&'a TaskRow),
}

/// One entry in a project's preorder outline.
pub(crate) struct PlanEntry<'a> {
    pub(crate) kind: PlanKind<'a>,
    pub(crate) parent: Option<usize>,
    pub(crate) outline_level: u8,
    pub(crate) wbs_code: Option<String>,
}

enum Pending<'a> {
    Summary(&'a WbsRow, usize, u8),
    Activity(&'a TaskRow, usize, u8),
}

/// Build one selected project's WBS/activity outline without recursion.
#[expect(
    clippy::too_many_lines,
    reason = "one linear WBS walk that avoids recursion with an explicit stack; splitting it would only move the same locals into arguments"
)]
pub(crate) fn plan<'a>(
    data: &'a xer::Data,
    project: &ProjectRow,
    notes: &mut Notes,
) -> Result<Vec<PlanEntry<'a>>, ImportError> {
    let rows: Vec<_> = data
        .wbs_rows
        .iter()
        .filter(|row| row.proj_id == project.proj_id)
        .collect();
    let activities: Vec<_> = data
        .task_rows
        .iter()
        .filter(|row| row.proj_id == project.proj_id)
        .collect();

    let mut by_id = HashMap::with_capacity(rows.len());
    for row in &rows {
        if by_id.insert(row.wbs_id, *row).is_some() {
            return Err(ImportError::DuplicateId {
                table: "PROJWBS".to_owned(),
                id: row.wbs_id,
            });
        }
    }

    if activities.is_empty() {
        notes.add(
            Section::Project,
            &format!("Project {}: it has no activities, so nothing was read from it; no dates are affected", project.proj_id),
            None,
        );
        notes.add_many(
            Section::Activity,
            "WBS elements with no activity under them were left out; no dates are affected",
            rows.len(),
        );
        return Ok(Vec::new());
    }

    let real_root = rows
        .iter()
        .copied()
        .find(|row| row.proj_node_flag == Some(true));
    if real_root.is_none() {
        notes.add(
            Section::Project,
            &format!("Project {}: the file has no top WBS element, so one is added above the rest; no dates are affected", project.proj_id),
            None,
        );
    }
    let root_id = real_root.map(|row| row.wbs_id);

    // `None` means the project root; `Some` names another real WBS row.
    let mut parents = HashMap::<u32, Option<u32>>::with_capacity(rows.len());
    for row in &rows {
        if Some(row.wbs_id) == root_id {
            continue;
        }
        let parent = row.parent_wbs_id.filter(|parent| {
            Some(*parent) == root_id || (*parent != row.wbs_id && by_id.contains_key(parent))
        });
        let parent = match parent {
            Some(parent) if Some(parent) != root_id => Some(parent),
            Some(_) => None,
            None => {
                notes.add(Section::Activity, "WBS elements whose parent is missing or loops back were moved to the top level; no dates are affected", None);
                None
            }
        };
        parents.insert(row.wbs_id, parent);
    }

    xer_wbs_code::break_parent_cycles(&mut parents, notes);
    let codes = Codes::new(real_root, &parents, &by_id);

    let mut direct = HashMap::<u32, Vec<&TaskRow>>::new();
    let mut root_activities = Vec::new();
    for activity in activities {
        if Some(activity.wbs_id) == root_id {
            root_activities.push(activity);
        } else if by_id.contains_key(&activity.wbs_id) {
            direct.entry(activity.wbs_id).or_default().push(activity);
        } else {
            let label = activity_label(activity);
            notes.add(
                Section::Activity,
                "Activities whose WBS element is not in the file were placed directly under the project; no dates are affected",
                Some(&label),
            );
            root_activities.push(activity);
        }
    }
    xer_activity_order::sort(&mut root_activities);
    for values in direct.values_mut() {
        xer_activity_order::sort(values);
    }

    let mut kept = HashSet::new();
    for wbs_id in direct.keys().copied() {
        let mut cursor = Some(wbs_id);
        while let Some(id) = cursor {
            if !kept.insert(id) {
                break;
            }
            cursor = parents.get(&id).copied().flatten();
        }
    }
    for row in &rows {
        if Some(row.wbs_id) != root_id && !kept.contains(&row.wbs_id) {
            notes.add(
                Section::Activity,
                "WBS elements with no activity under them were left out; no dates are affected",
                None,
            );
        }
    }

    let mut children = HashMap::<Option<u32>, Vec<&WbsRow>>::new();
    for row in rows {
        if Some(row.wbs_id) != root_id && kept.contains(&row.wbs_id) {
            let parent = parents.get(&row.wbs_id).copied().flatten();
            children.entry(parent).or_default().push(row);
        }
    }
    for values in children.values_mut() {
        values.sort_by(|left, right| compare_wbs(left, right));
    }

    let root_name = project
        .proj_short_name
        .clone()
        .unwrap_or_else(|| format!("Project {}", project.proj_id));
    let mut result = vec![PlanEntry {
        kind: PlanKind::Summary {
            uid: root_id.unwrap_or(project.proj_id),
            name: root_name,
        },
        parent: None,
        outline_level: 1,
        wbs_code: codes.root(),
    }];
    let mut pending = Vec::new();
    push_children(&mut pending, &root_activities, children.get(&None), 0, 2);

    while let Some(item) = pending.pop() {
        match item {
            Pending::Activity(activity, parent, level) => {
                ensure_depth(level)?;
                result.push(PlanEntry {
                    kind: PlanKind::Activity(activity),
                    parent: Some(parent),
                    outline_level: level,
                    wbs_code: codes.of_activity(activity.wbs_id),
                });
            }
            Pending::Summary(row, parent, level) => {
                ensure_depth(level)?;
                let index = result.len();
                result.push(PlanEntry {
                    kind: PlanKind::Summary {
                        uid: row.wbs_id,
                        name: row
                            .wbs_name
                            .clone()
                            .or_else(|| row.wbs_short_name.clone())
                            .unwrap_or_default(),
                    },
                    parent: Some(parent),
                    wbs_code: codes.of(row.wbs_id),
                    outline_level: level,
                });
                let child_level = level.checked_add(1).ok_or(ImportError::WbsTooDeep)?;
                push_children(
                    &mut pending,
                    direct.get(&row.wbs_id).map_or(&[], Vec::as_slice),
                    children.get(&Some(row.wbs_id)),
                    index,
                    child_level,
                );
            }
        }
    }
    Ok(result)
}

fn push_children<'a>(
    pending: &mut Vec<Pending<'a>>,
    activities: &[&'a TaskRow],
    children: Option<&Vec<&'a WbsRow>>,
    parent: usize,
    level: u8,
) {
    if let Some(children) = children {
        for child in children.iter().rev() {
            pending.push(Pending::Summary(child, parent, level));
        }
    }
    for activity in activities.iter().rev() {
        pending.push(Pending::Activity(activity, parent, level));
    }
}

fn compare_wbs(left: &WbsRow, right: &WbsRow) -> Ordering {
    left.seq_num
        .is_none()
        .cmp(&right.seq_num.is_none())
        .then_with(|| left.seq_num.unwrap_or(0).cmp(&right.seq_num.unwrap_or(0)))
        .then_with(|| left.wbs_short_name.cmp(&right.wbs_short_name))
        .then_with(|| left.wbs_id.cmp(&right.wbs_id))
}

fn ensure_depth(level: u8) -> Result<(), ImportError> {
    if level > 250 {
        Err(ImportError::WbsTooDeep)
    } else {
        Ok(())
    }
}

fn activity_label(activity: &TaskRow) -> String {
    activity
        .task_code
        .clone()
        .unwrap_or_else(|| activity.task_id.to_string())
}

#[cfg(test)]
mod tests;
