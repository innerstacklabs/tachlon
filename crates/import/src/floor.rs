//! The project-start floor both importers share.

use engine::{Schedule, TaskId};

use crate::{ImportError, ImportedTask};

/// One task's start inputs: its project's start and its own constraint date, both instants from
/// the schedule origin.
pub(crate) struct Floor {
    pub(crate) id: TaskId,
    /// `None` when the project has no start date: the task is left unfloored.
    pub(crate) project_start: Option<i64>,
    pub(crate) own_snet: Option<i64>,
}

/// The start-no-earlier-than instant for a task: its project's start, or its own constraint date
/// when that is later (ruling devteam-1-64: a constraint earlier than the project start is
/// overridden by the floor).
pub(crate) fn floored_snet(project_start: i64, own_snet: Option<i64>) -> i64 {
    own_snet.map_or(project_start, |own| own.max(project_start))
}

/// Once the file's project start is known (MS Project states it in the header, which can come
/// after a task), give each task's `Floor` and its `ImportedTask` that start. The two slices run
/// in the same order.
pub(crate) fn set_project_start(
    floors: &mut [Floor],
    imported: &mut [ImportedTask],
    start: Option<i64>,
) {
    for (floor, task) in floors.iter_mut().zip(imported) {
        floor.project_start = start;
        task.floor = start;
    }
}

/// Floor every task at `max(project start, own constraint date)`, whatever links it has. A task
/// whose project has no start date keeps its own constraint, if any.
pub(crate) fn apply(floors: &[Floor], schedule: &mut Schedule) -> Result<(), ImportError> {
    for floor in floors {
        let Some(start) = floor.project_start else {
            continue;
        };
        let task =
            schedule
                .tasks()
                .get(floor.id.0 as usize)
                .ok_or_else(|| ImportError::Missing {
                    element: "Task".to_owned(),
                })?;
        let final_start = floored_snet(start, floor.own_snet);
        if task.snet != Some(final_start) {
            schedule.set_snet(floor.id, Some(final_start))?;
        }
    }
    Ok(())
}
