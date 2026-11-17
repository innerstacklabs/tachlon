//! The parent links of an XER project's WBS nodes: cycle breaking, and the dotted WBS code.
//!
//! A WBS node's code is the `wbs_short_name` of each node on its path from the project's real
//! root down to the node itself, joined with a dot (`HAND.1.2`). A node with no short name adds no
//! segment. A project with no real root row has no root segment.

use std::collections::{HashMap, HashSet};

use xer::WbsRow;

use crate::notes::{Notes, Section};

/// The WBS code of every non-root WBS node of one project, and of its root.
pub(crate) struct Codes {
    root: Option<String>,
    by_wbs: HashMap<u32, String>,
}

impl Codes {
    /// `parents` maps each non-root node to its parent (`None` for a child of the root) and holds no
    /// cycle (see [`break_parent_cycles`]); `by_id` holds every WBS row of the project.
    pub(crate) fn new(
        root: Option<&WbsRow>,
        parents: &HashMap<u32, Option<u32>>,
        by_id: &HashMap<u32, &WbsRow>,
    ) -> Self {
        let segment = |row: &WbsRow| {
            row.wbs_short_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        };
        let root = root.and_then(segment);
        let mut by_wbs = HashMap::with_capacity(parents.len());
        for &start in parents.keys() {
            let mut segments = Vec::new();
            let mut cursor = Some(start);
            // The step bound only guards a map that still holds a cycle.
            while let Some(id) = cursor.filter(|_| segments.len() <= parents.len()) {
                segments.extend(by_id.get(&id).and_then(|row| segment(row)));
                cursor = parents.get(&id).copied().flatten();
            }
            segments.extend(root.clone());
            segments.reverse();
            if !segments.is_empty() {
                by_wbs.insert(start, segments.join("."));
            }
        }
        Self { root, by_wbs }
    }

    /// The code of the project's root, or `None` when the root has no short name or is synthetic.
    pub(crate) fn root(&self) -> Option<String> {
        self.root.clone()
    }

    /// The code of WBS node `wbs_id`.
    pub(crate) fn of(&self, wbs_id: u32) -> Option<String> {
        self.by_wbs.get(&wbs_id).cloned()
    }

    /// The code of the node an activity sits in: the root's for an activity on the root or on a
    /// node the project lacks.
    pub(crate) fn of_activity(&self, wbs_id: u32) -> Option<String> {
        self.by_wbs.get(&wbs_id).cloned().or_else(|| self.root())
    }
}

pub(crate) fn break_parent_cycles(parents: &mut HashMap<u32, Option<u32>>, notes: &mut Notes) {
    let ids: Vec<u32> = parents.keys().copied().collect();
    let mut done = HashSet::new();
    for start in ids {
        if done.contains(&start) {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = HashMap::new();
        let mut cursor = Some(start);
        while let Some(id) = cursor {
            if done.contains(&id) {
                break;
            }
            if let Some(position) = positions.get(&id).copied() {
                for cycle_id in path.iter().skip(position).copied() {
                    parents.insert(cycle_id, None);
                    notes.add(Section::Activity, "WBS elements whose parent is missing or loops back were moved to the top level; no dates are affected", None);
                }
                break;
            }
            positions.insert(id, path.len());
            path.push(id);
            cursor = parents.get(&id).copied().flatten();
        }
        done.extend(path);
    }
}
