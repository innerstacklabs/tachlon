//! The resource names of each task (gs#703), kept for display only. A side table beside
//! [`TaskMeta`](crate::meta::TaskMeta), indexed by task id: no date, float or link depends on it,
//! and no edit, undo or redo changes it. A task added after the open has none.

use crate::Session;
use engine::TaskId;

/// A resource on a task: its name and its units, a whole percent from 1 to 1000 (gantt#1875).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assigned {
    pub name: String,
    pub units: u16,
}

/// The units of every resource an MS Project file gives a task today.
pub const FULL_UNITS: u16 = 100;

/// `names` as resources at [`FULL_UNITS`].
pub(crate) fn full_units(names: Vec<String>) -> Vec<Assigned> {
    (names.into_iter())
        .map(|name| Assigned {
            name,
            units: FULL_UNITS,
        })
        .collect()
}

impl Session {
    /// Keep the resources of each task, as `Loaded::resources`. Display only.
    pub fn with_assignments(mut self, resources: Vec<Vec<Assigned>>) -> Session {
        self.resources = resources;
        self
    }

    /// Keep the resource names of each task, each at [`FULL_UNITS`].
    pub fn with_resources(self, names: Vec<Vec<String>>) -> Session {
        self.with_assignments(names.into_iter().map(full_units).collect())
    }

    /// What the file gave `task`'s resources, in its order; empty when it gave none or the task is new.
    pub fn resources(&self, task: TaskId) -> &[Assigned] {
        self.resources
            .get(task.0 as usize)
            .map_or(&[], Vec::as_slice)
    }
}
