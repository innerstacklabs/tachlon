use std::collections::HashMap;

use engine::{Schedule, Task, TaskId};
use xer::{StatusCode, TaskRow};

use crate::floor::{self, Floor};
use crate::notes::Notes;
use crate::xer_calendars::XerCalendars;
use crate::xer_import::{SelectedProject, instant_from_origin};
use crate::xer_progress::{self, PercentColumns};
use crate::xer_wbs::{PlanEntry, PlanKind};
use crate::{ImportError, ImportedTask};

mod actuals;
mod constraints;
mod duration;
#[cfg(test)]
mod tests;

use actuals::Actuals;
use constraints::{Constraints, constraint};
use duration::{apply_task_type, duration};

/// Link-import metadata parallel to the engine task list.
pub(crate) struct TaskInfo {
    pub(crate) is_activity: bool,
    pub(crate) label: String,
}

/// Activity results retained for relationship import and project-start handling.
pub(crate) struct BuiltActivities {
    pub(crate) imported: Vec<ImportedTask>,
    pub(crate) by_task_id: HashMap<u32, TaskId>,
    pub(crate) task_info: Vec<TaskInfo>,
    starts: Vec<Floor>,
}

/// Turn selected-project preorder plans into engine tasks and import metadata.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pass in which each plan entry becomes a task; splitting it would only move the same locals into arguments"
)]
pub(crate) fn build(
    projects: &[SelectedProject<'_>],
    plans: &[Vec<PlanEntry<'_>>],
    schedule: &mut Schedule,
    calendars: &mut XerCalendars<'_>,
    origin_day: i64,
    columns: PercentColumns,
    notes: &mut Notes,
) -> Result<BuiltActivities, ImportError> {
    let total = plans
        .iter()
        .fold(0usize, |count, plan| count.saturating_add(plan.len()));
    let mut imported = Vec::with_capacity(total);
    let mut by_task_id = HashMap::new();
    let mut task_info = Vec::with_capacity(total);
    let mut starts = Vec::new();

    for (project, plan) in projects.iter().zip(plans) {
        let mut plan_ids = Vec::with_capacity(plan.len());
        for entry in plan {
            let parent = entry.parent.and_then(|index| plan_ids.get(index).copied());
            let (id, metadata, info, start) = match &entry.kind {
                PlanKind::Summary { uid, name } => {
                    let id = schedule.add_task(Task {
                        duration: 0,
                        calendar: project.default_calendar,
                        snet: None,
                        limit: engine::Limit::default(),
                        deleted: false,
                        actual_start: None,
                        actual_finish: None,
                        remaining: None,
                    })?;
                    (
                        id,
                        ImportedTask {
                            uid: *uid,
                            code: None,
                            name: name.clone(),
                            outline_level: entry.outline_level,
                            parent,
                            file_start: None,
                            file_finish: None,
                            in_progress: false,
                            actual_start: None,
                            actual_finish: None,
                            remaining: None,
                            baseline_start: None,
                            baseline_finish: None,
                            progress: None,
                            physical: None,
                            activity_codes: Vec::new(),
                            wbs_code: entry.wbs_code.clone(),
                            floor: None,
                            deadline: None,
                            resource_names: Vec::new(),
                        },
                        TaskInfo {
                            is_activity: false,
                            label: uid.to_string(),
                        },
                        None,
                    )
                }
                PlanKind::Activity(activity) => {
                    let label = activity_label(activity);
                    let calendar = calendars.for_activity(
                        activity.clndr_id,
                        project.default_calendar,
                        &label,
                        schedule,
                        notes,
                    )?;
                    let mut duration = duration(activity, &label, notes)?;
                    apply_task_type(activity, &label, &mut duration, notes);
                    let Constraints { snet, limit } =
                        constraint(activity, &label, origin_day, notes)?;
                    let actuals = Actuals::of(activity, origin_day, &label, notes);
                    let id = schedule.add_task(Task {
                        duration,
                        calendar,
                        snet,
                        limit,
                        deleted: false,
                        actual_start: None,
                        actual_finish: None,
                        remaining: None,
                    })?;
                    by_task_id.insert(activity.task_id, id);
                    (
                        id,
                        ImportedTask {
                            uid: activity.task_id,
                            code: activity.task_code.clone(),
                            name: activity
                                .task_name
                                .clone()
                                .or_else(|| activity.task_code.clone())
                                .unwrap_or_default(),
                            outline_level: entry.outline_level,
                            parent,
                            file_start: activity
                                .early_start_date
                                .and_then(|minutes| instant_from_origin(minutes, origin_day)),
                            file_finish: activity
                                .early_end_date
                                .and_then(|minutes| instant_from_origin(minutes, origin_day)),
                            in_progress: matches!(activity.status_code, Some(StatusCode::Active)),
                            actual_start: actuals.start,
                            actual_finish: actuals.finish,
                            remaining: actuals.remaining,
                            baseline_start: None,
                            baseline_finish: None,
                            progress: xer_progress::percent(
                                columns,
                                project.project,
                                activity,
                                notes,
                            ),
                            physical: xer_progress::physical(activity, notes),
                            activity_codes: Vec::new(),
                            wbs_code: entry.wbs_code.clone(),
                            floor: project.start_instant,
                            deadline: None,
                            resource_names: Vec::new(),
                        },
                        TaskInfo {
                            is_activity: true,
                            label: label.clone(),
                        },
                        Some(Floor {
                            id,
                            project_start: project.start_instant,
                            own_snet: snet,
                        }),
                    )
                }
            };
            plan_ids.push(id);
            imported.push(metadata);
            task_info.push(info);
            if let Some(start) = start {
                starts.push(start);
            }
        }
    }

    Ok(BuiltActivities {
        imported,
        by_task_id,
        task_info,
        starts,
    })
}

/// Floor every activity at its project's start, retaining a later activity constraint.
pub(crate) fn apply_project_start(
    built: &BuiltActivities,
    schedule: &mut Schedule,
) -> Result<(), ImportError> {
    // Ruling devteam-1-64 (a): per P6 forward-pass behaviour, to confirm on a real P6 export (gs#116 corpus).
    floor::apply(&built.starts, schedule)
}

fn activity_label(activity: &TaskRow) -> String {
    activity
        .task_code
        .as_deref()
        .map_or_else(|| activity.task_id.to_string(), ascii)
}

fn ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii() { character } else { '?' })
        .collect()
}
