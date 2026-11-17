//! The Resource row filter (gs#703): the tasks that name one resource, and the summaries above them.

use crate::{RowFilter, Session};
use engine::TaskId;
use std::collections::BTreeSet;
use std::sync::Arc;

impl Session {
    /// The resource the row filter keeps, when that filter is on.
    pub fn resource_filter(&self) -> Option<&str> {
        match &self.row_filter {
            RowFilter::Resource(name) => Some(name),
            _ => None,
        }
    }

    /// Keep only the leaves that name the resource `name` ([`Session::resources`]), and the
    /// summaries that contain one; `None` turns the filter off and leaves a different filter on.
    /// It replaces any other row filter. Returns false, changing nothing, when no live task names
    /// that resource, there is no outline, or a bar press is active.
    pub fn set_resource_filter(&mut self, name: Option<&str>) -> bool {
        let wanted = match (name, &self.row_filter) {
            (Some(name), _) if self.resource_names().iter().any(|known| known == name) => {
                RowFilter::Resource(Arc::from(name))
            }
            (Some(_), _) => return false,
            (None, RowFilter::Resource(_)) => RowFilter::Off,
            (None, other) => other.clone(),
        };
        self.set_row_filter(wanted)
    }

    /// The resource names the live tasks carry, in order of name, each once. A name the file left
    /// empty is not listed.
    pub fn resource_names(&self) -> Vec<String> {
        let found: BTreeSet<&str> = self
            .resources
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                u32::try_from(*index).is_ok_and(|id| !self.schedule.is_deleted(TaskId(id)))
            })
            .flat_map(|(_, held)| held.iter().map(|a| a.name.as_str()))
            .filter(|name| !name.is_empty())
            .collect();
        found.into_iter().map(str::to_owned).collect()
    }
}
