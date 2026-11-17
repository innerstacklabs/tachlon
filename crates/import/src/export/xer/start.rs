//! Each project's `plan_start_date`: the instant the reader floors the project's tasks at.

use engine::Instant;

/// The start of each of `count` projects, as instants from the schedule origin. A given project
/// start (`Imported::project_start`) is every project's; without one a project starts at the
/// earliest start-no-earlier-than among its tasks, and at the origin when it has none. `snets`
/// pairs a task's project index with its constraint.
pub(super) fn project_starts(
    count: usize,
    given: Option<Instant>,
    snets: impl Iterator<Item = (usize, Option<Instant>)>,
) -> Vec<Instant> {
    if let Some(start) = given {
        return vec![start; count];
    }
    let mut starts: Vec<Option<Instant>> = vec![None; count];
    for (project, snet) in snets {
        if let (Some(snet), Some(slot)) = (snet, starts.get_mut(project)) {
            *slot = Some(slot.map_or(snet, |earliest| earliest.min(snet)));
        }
    }
    starts.into_iter().map(|start| start.unwrap_or(0)).collect()
}
