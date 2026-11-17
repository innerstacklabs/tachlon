//! What the load path reads out of a file beside its schedule: the baseline it saved, and the
//! check of its in-progress dates against the solution.

use crate::baseline::{Baseline, BaselineTask};
use engine::Solution;

/// Most activity IDs the file-dates line names.
const FILE_DATES_IDS: usize = 5;

/// What the file-dates check keeps of one task once the import is freed: its row, the dates the
/// file states and the activity ID the line would name.
pub(super) struct FileDates {
    index: usize,
    start: Option<engine::Instant>,
    finish: Option<engine::Instant>,
    id: String,
}

/// The tasks the file-dates check compares: a dated activity in progress.
pub(super) fn file_dates(tasks: &[import::ImportedTask]) -> Vec<FileDates> {
    tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| {
            task.in_progress && (task.file_start.is_some() || task.file_finish.is_some())
        })
        .map(|(index, task)| FileDates {
            index,
            start: task.file_start,
            finish: task.file_finish,
            id: file_dates_id(task),
        })
        .collect()
}

/// The in-progress activities whose early dates in the file fall on a different calendar day
/// than the engine computed with the actuals and data date applied (gs#87 plan section 5); only
/// a dated activity in progress is compared (see [`file_dates`]), and only an end the file states.
pub(super) fn file_dates_line(tasks: &[FileDates], solution: &Solution) -> Option<String> {
    let mut compared = 0usize;
    let mut differing = 0usize;
    let mut first = Vec::new();
    for task in tasks {
        let Some(dates) = solution.dates.get(task.index) else {
            continue;
        };
        compared += 1;
        let day = |instant: engine::Instant| instant.div_euclid(engine::DAY);
        let differs = task.start.is_some_and(|start| day(start) != day(dates.es))
            || task
                .finish
                .is_some_and(|finish| day(finish) != day(dates.ef));
        if differs {
            differing += 1;
            if first.len() < FILE_DATES_IDS {
                first.push(task.id.as_str());
            }
        }
    }
    if differing == 0 {
        return None;
    }
    Some(format!(
        "in-progress activities: {differing} of {compared} show different dates in the file than the engine computed (first: {})",
        first.join(", ")
    ))
}

/// The activity ID the line names: the P6 code, else the source UID; control characters become spaces.
fn file_dates_id(task: &import::ImportedTask) -> String {
    match &task.code {
        Some(code) => code
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect(),
        None => task.uid.to_string(),
    }
}

/// The baseline the file saved (MS Project's Baseline 0), indexed like the tasks, or `None` when
/// no task states one. A task has an entry only when the file gives both its baseline start and
/// finish; the file states no float, so the entries hold none. The project finish is the latest
/// baseline finish, as MS Project reports its own.
pub(super) fn file_baseline(tasks: &[import::ImportedTask]) -> Option<Baseline> {
    let entries: Vec<Option<BaselineTask>> = tasks
        .iter()
        .map(|task| match (task.baseline_start, task.baseline_finish) {
            (Some(es), Some(ef)) => Some(BaselineTask {
                es,
                ef,
                float: None,
            }),
            _ => None,
        })
        .collect();
    let project_finish = entries.iter().flatten().map(|entry| entry.ef).max()?;
    Some(Baseline::from_parts(entries, project_finish))
}
