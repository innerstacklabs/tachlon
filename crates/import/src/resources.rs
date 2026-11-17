//! The resource assignments of an MS Project file: the names kept on each task, and the one note
//! for them.

use std::collections::HashMap;

use crate::ImportedTask;
use crate::dom::Node;
use crate::fields::optional_i64;

/// Set `resource_names` on each task in `assigned` (its `TaskUID` and the `ResourceUID` of each
/// of its assignments): the `Name` of the `Resource` with that `UID` in `root`'s `Resources`.
/// A resource with no name, no readable `UID` or none listed adds no name.
pub(crate) fn name_tasks(
    tasks: &mut [ImportedTask],
    assigned: &HashMap<u32, Vec<i64>>,
    root: &Node,
) {
    if assigned.is_empty() {
        return;
    }
    let mut names: HashMap<i64, &str> = HashMap::new();
    for resource in root
        .child("Resources")
        .into_iter()
        .flat_map(|resources| resources.children_named("Resource"))
    {
        let uid = optional_i64(resource, "UID").ok().flatten();
        let name = resource.child_text("Name").filter(|name| !name.is_empty());
        if let (Some(uid), Some(name)) = (uid, name) {
            names.entry(uid).or_insert(name);
        }
    }
    for task in tasks {
        if let Some(uids) = assigned.get(&task.uid) {
            task.resource_names = uids
                .iter()
                .filter_map(|uid| names.get(uid))
                .map(|name| (*name).to_owned())
                .collect();
        }
    }
}

/// The note for the `tasks` tasks that have a resource assignment, however many there are.
pub(crate) fn note(tasks: usize) -> String {
    let (noun, verb) = if tasks == 1 {
        ("task", "has")
    } else {
        ("tasks", "have")
    };
    format!(
        "{tasks} {noun} {verb} resource assignments; only the resource names were read, and no dates are affected."
    )
}
