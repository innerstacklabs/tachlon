use std::collections::{HashMap, HashSet};

use engine::{Dependency, LagCalendar, LinkType, Schedule};
use xer::PredType;

use crate::ImportError;
use crate::notes::{Notes, Section};
use crate::range;
use crate::xer_activities::BuiltActivities;
use crate::xer_import::SelectedProject;

/// P6's own default when `sched_calendar_on_relationship_lag` is absent is not documented by
/// Oracle (gs#148 ruling 3 as amended): Predecessor, not today's Successor.
const DEFAULT_LAG_CALENDAR: LagCalendar = LagCalendar::Predecessor;
const DEFAULT_LAG_NAME: &str = "Predecessor";
const DEFAULT_LAG_WHY: &str = "the default; Oracle does not document it";

/// Import XER relationships in file order.
pub(crate) fn build(
    data: &xer::Data,
    projects: &[SelectedProject<'_>],
    built: &BuiltActivities,
    schedule: &mut Schedule,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    apply_lag_calendar(data, projects, schedule, notes)?;
    let mut task_projects = HashMap::with_capacity(data.task_rows.len());
    for task in &data.task_rows {
        task_projects.insert(task.task_id, task.proj_id);
    }

    let selected_projects: HashSet<u32> = projects
        .iter()
        .map(|project| project.project.proj_id)
        .collect();
    let mut seen = HashSet::with_capacity(data.pred_rows.len());
    for row in &data.pred_rows {
        let label = successor_label(built, row.task_id);
        let Some(&successor_project) = task_projects.get(&row.task_id) else {
            notes.add(
                Section::Relationship,
                "Links to an activity that is not in the file were not read; dates can differ from the file's",
                Some(&label),
            );
            continue;
        };
        if !task_projects.contains_key(&row.pred_task_id) {
            notes.add(
                Section::Relationship,
                "Links to an activity that is not in the file were not read; dates can differ from the file's",
                Some(&label),
            );
            continue;
        }
        if row.task_id == row.pred_task_id {
            notes.add(
                Section::Relationship,
                "Links from an activity to itself were not read; no dates are affected",
                Some(&label),
            );
            continue;
        }

        // A link with both ends in unselected projects belongs to those projects (already
        // counted as unselected); one end in a selected project makes it external.
        let (Some(&successor), Some(&predecessor)) = (
            built.by_task_id.get(&row.task_id),
            built.by_task_id.get(&row.pred_task_id),
        ) else {
            let predecessor_project = task_projects.get(&row.pred_task_id).copied();
            if selected_projects.contains(&successor_project)
                || predecessor_project.is_some_and(|project| selected_projects.contains(&project))
            {
                notes.add(Section::Project, "Links to activities in another project were not read; dates can differ from the file's", None);
            }
            continue;
        };
        let Some(kind) = link_type(row.pred_type.as_ref(), &label, notes) else {
            continue;
        };
        // `lag_hr_cnt` is unverified on a real P6 export (ruling 3, gs#116 corpus).
        let lag = lag(row.lag_hr_cnt, row.task_id, &label, notes)?;

        if !seen.insert((successor, predecessor, kind)) {
            notes.add(
                Section::Relationship,
                "The same link appears more than once; every copy was read and no dates are affected",
                Some(&label),
            );
        }
        schedule.add_dependency(Dependency {
            pred: predecessor,
            succ: successor,
            kind,
            lag,
        })?;
    }
    Ok(())
}

fn link_type(kind: Option<&PredType>, label: &str, notes: &mut Notes) -> Option<LinkType> {
    // PR_* spellings are unverified on a real P6 export (ruling 3, gs#116 corpus).
    match kind {
        Some(PredType::FinishToStart) => Some(LinkType::FS),
        Some(PredType::StartToStart) => Some(LinkType::SS),
        Some(PredType::FinishToFinish) => Some(LinkType::FF),
        Some(PredType::StartToFinish) => Some(LinkType::SF),
        Some(PredType::Other(code)) => {
            notes.add(
                Section::Relationship,
                &format!("Links of type {}, which this import does not know, were not read; dates can differ from the file's", ascii(code)),
                Some(label),
            );
            None
        }
        None => {
            notes.add(
                Section::Relationship,
                "Links with no type were not read; dates can differ from the file's",
                Some(label),
            );
            None
        }
    }
}

fn lag(
    hours: Option<f64>,
    successor_id: u32,
    label: &str,
    notes: &mut Notes,
) -> Result<i64, ImportError> {
    let Some(hours) = hours else {
        return Ok(0);
    };
    let minutes = hours * 60.0;
    // Negative lag is a valid lead; only a non-finite value fails.
    if !minutes.is_finite() {
        return Err(ImportError::Invalid {
            element: format!("relationship {successor_id} lag"),
            value: hours.to_string(),
        });
    }
    let rounded = minutes.round();
    if (minutes - rounded).abs() > 1e-6 {
        notes.add(
            Section::Relationship,
            "A link's lag was rounded to the whole minute; dates can differ by under a minute",
            Some(label),
        );
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the check above leaves a finite value, and `as` saturates at either end of the range instead of wrapping; `range::lag` then bounds it"
    )]
    let whole = rounded as i64;
    Ok(range::lag(whole, label, notes))
}

/// What the file says about the lag calendar of one selected project.
struct LagSetting {
    calendar: LagCalendar,
    /// `Some` when the value was absent or not recognised, so the default stands in.
    note: Option<String>,
}

/// Read one project's `sched_calendar_on_relationship_lag`. Exact spellings only (after trimming
/// spaces); anything else is not guessed at and takes the default with a note.
fn lag_setting(data: &xer::Data, project: &SelectedProject<'_>) -> LagSetting {
    let proj_id = project.project.proj_id;
    let raw = data
        .sched_options_rows
        .iter()
        .find(|row| row.proj_id == Some(proj_id))
        .and_then(|row| row.sched_calendar_on_relationship_lag.as_deref())
        .map_or("", str::trim);
    let calendar = match raw {
        "rcal_Successor" => LagCalendar::Successor,
        "rcal_Predecessor" => LagCalendar::Predecessor,
        "rcal_ProjDefault" => LagCalendar::Fixed(project.default_calendar),
        "rcal_24Hour" => LagCalendar::TwentyFourHour,
        "" => {
            return LagSetting {
                calendar: DEFAULT_LAG_CALENDAR,
                note: Some(format!(
                    "The file does not say which calendar a link's lag follows, so the {DEFAULT_LAG_NAME} calendar is used ({DEFAULT_LAG_WHY}); dates can differ from the file's"
                )),
            };
        }
        other => {
            let shown: String = ascii(other).chars().take(40).collect();
            return LagSetting {
                calendar: DEFAULT_LAG_CALENDAR,
                note: Some(format!(
                    "The lag calendar {shown} is not one this import knows, so the {DEFAULT_LAG_NAME} calendar is used ({DEFAULT_LAG_WHY}); dates can differ from the file's"
                )),
            };
        }
    };
    LagSetting {
        calendar,
        note: None,
    }
}

/// Apply the lag calendar to the schedule: one choice for the whole schedule (ruling 1, gs#148).
/// The first selected project's setting wins; every project that resolves differently is named in
/// one note. A project that states nothing, or something unknown, takes the default and says so.
fn apply_lag_calendar(
    data: &xer::Data,
    projects: &[SelectedProject<'_>],
    schedule: &mut Schedule,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    let settings: Vec<LagSetting> = projects
        .iter()
        .map(|project| lag_setting(data, project))
        .collect();
    for (project, setting) in projects.iter().zip(&settings) {
        if let Some(note) = &setting.note {
            let id = project.project.proj_id.to_string();
            notes.add(Section::Project, note, Some(&id));
        }
    }
    let Some(first) = settings.first() else {
        return Ok(());
    };
    for (project, setting) in projects.iter().zip(&settings).skip(1) {
        if setting.calendar != first.calendar {
            let id = project.project.proj_id.to_string();
            notes.add(
                Section::Project,
                "The lag calendar differs from the first project's; the first project's is used for all, so dates can differ from the file's",
                Some(&id),
            );
        }
    }
    schedule.set_lag_calendar(first.calendar)?;
    Ok(())
}

fn successor_label(built: &BuiltActivities, task_id: u32) -> String {
    built
        .by_task_id
        .get(&task_id)
        .and_then(|id| built.task_info.get(id.0 as usize))
        .filter(|info| info.is_activity)
        .map_or_else(|| task_id.to_string(), |info| info.label.clone())
}

fn ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii() { character } else { '?' })
        .collect()
}

#[cfg(test)]
mod tests;
