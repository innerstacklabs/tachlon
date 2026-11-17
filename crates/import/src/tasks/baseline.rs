//! The task's saved baseline: the `Baseline` element numbered 0, read for the baseline bar.

use engine::Instant;

use super::relative_instant;
use crate::dom::Node;

/// The `Start` and `Finish` of the task's `Baseline` element numbered 0, as instants from the
/// schedule origin; the other baselines (numbers 1 to 10) are not read. MS Project saves
/// either date without the other, so each is read on its own: a task whose baseline 0 states
/// only a cost or a duration, or no baseline 0 at all, has neither. An unreadable date is `None`.
/// A date before the origin is a negative instant, as for the file's own `Start` and `Finish`.
pub(super) fn baseline_dates(node: &Node, origin_day: i64) -> (Option<Instant>, Option<Instant>) {
    let Some(baseline) = node
        .children_named("Baseline")
        .find(|baseline| baseline.child_text("Number").map(str::trim) == Some("0"))
    else {
        return (None, None);
    };
    let instant = |element: &str| {
        baseline
            .child_text(element)
            .filter(|value| !value.is_empty())
            .and_then(|value| relative_instant(value, origin_day, element).ok())
    };
    (instant("Start"), instant("Finish"))
}
