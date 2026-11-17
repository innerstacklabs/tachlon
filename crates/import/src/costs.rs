//! The one note for task costs, which the MS Project reader does not read: it counts the tasks that state one.

use crate::ImportError;
use crate::dom::Node;
use crate::fields::flag;

/// Whether the task states a nonzero `Cost`, `FixedCost` or `ActualCost`. A summary's cost rolls up
/// its children's, so a summary never counts.
pub(crate) fn states_cost(node: &Node) -> Result<bool, ImportError> {
    Ok(flag(node, "Summary")? != Some(true)
        && ["Cost", "FixedCost", "ActualCost"].iter().any(|name| {
            node.child_text(name)
                .is_some_and(|value| value.bytes().any(|digit| matches!(digit, b'1'..=b'9')))
        }))
}

/// The note for the `tasks` tasks that state a cost, or none when no task does.
pub(crate) fn note(tasks: usize) -> Option<String> {
    let (noun, verb) = if tasks == 1 {
        ("task", "states")
    } else {
        ("tasks", "state")
    };
    (tasks > 0).then(|| {
        format!("{tasks} {noun} {verb} a cost; costs were not read, and no dates are affected.")
    })
}
