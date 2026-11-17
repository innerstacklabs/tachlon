//! The task's `Deadline`, kept for the day the session uses it.

use engine::Instant;

use crate::dom::Node;
use crate::tasks::relative_instant;

/// The task's `Deadline` as an instant from the schedule origin, negative when it falls before it.
/// `None` when the task states none or one that is not a date (that one earns [`NOT_READ`]).
/// No date moves.
pub(crate) fn read(node: &Node, origin_day: i64) -> Option<Instant> {
    node.child_text("Deadline")
        .filter(|value| !value.is_empty())
        .and_then(|value| relative_instant(value, origin_day, "Deadline").ok())
}

/// The counted note for the tasks whose `Deadline` is not a date.
pub(crate) const NOT_READ: &str =
    "Tasks with a deadline that is not a date: the deadline was not read; no dates are affected";
