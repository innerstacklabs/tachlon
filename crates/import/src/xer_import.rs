use std::collections::HashSet;

use engine::{CalendarId, Schedule};
use xer::ProjectRow;

use crate::notes::{Notes, Section, note_actuals_after_data_date};
use crate::range;
use crate::units::{days_from_civil, format_minutes, weekday_of};
use crate::xer_activities::{apply_project_start, build};
use crate::xer_calendars::XerCalendars;
use crate::xer_codes;
use crate::xer_links;
use crate::xer_progress::PercentColumns;
use crate::xer_wbs;
use crate::{ImportError, Imported};

mod file_notes;
mod select;

use file_notes::{file_notes, note_activities_without_project};
use select::select_projects;
#[cfg(test)]
mod tests;

/// Which exported projects to open from an XER file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProjectSelection {
    /// Open the only candidate project, or ask the caller to choose.
    #[default]
    Unspecified,
    /// Open every candidate project.
    All,
    /// Open the named project identifiers, in file order.
    Only(Vec<u32>),
}

pub(crate) struct SelectedProject<'a> {
    pub(crate) project: &'a ProjectRow,
    pub(crate) default_calendar: CalendarId,
    pub(crate) start_instant: Option<i64>,
}

/// Parse a Primavera P6 XER document into an engine schedule.
///
/// # Errors
///
/// Returns an [`ImportError`] when the file is not a readable XER document, a value is out of
/// range, or `selection` does not name a project of the file.
pub fn parse_xer(bytes: &[u8], selection: &ProjectSelection) -> Result<Imported, ImportError> {
    let parsed = xer::parse(bytes).map_err(xer_error)?;
    let data = xer::read(&parsed).map_err(xer_error)?;
    if data.project_rows.is_empty() {
        return Err(ImportError::NoProjects);
    }
    check_ids(&data)?;
    check_nonfinite_durations(&parsed)?;
    check_nonfinite_lags(&parsed)?;

    let mut notes = Notes::new();
    file_notes(&data, &mut notes);
    let selected = select_projects(&data, selection, &mut notes)?;
    note_activities_without_project(&data, &mut notes);
    let origin_day = origin_day(&selected, &mut notes)?;
    let data_date = data_date(&selected, origin_day, &mut notes);
    let mut schedule = Schedule::new();
    let mut calendars = XerCalendars::new(&data, origin_day);
    calendars.report_warnings(&selected, &mut notes);

    let mut projects = Vec::with_capacity(selected.len());
    for project in selected {
        let default_calendar = calendars.project_default(project, &mut schedule, &mut notes)?;
        let start_instant = project
            .plan_start_date
            .map(|minutes| relative_instant(minutes, origin_day))
            .transpose()?;
        if start_instant.is_none() {
            notes.add(
                Section::Project,
                "The project has no start date, so its activities are not held back to a project start and their dates can differ from the file's",
                Some(&project.proj_id.to_string()),
            );
        }
        projects.push(SelectedProject {
            project,
            default_calendar,
            start_instant,
        });
    }
    if !data.sched_options_rows.is_empty() {
        notes.add(
            Section::Project,
            "Scheduling options other than the lag calendar were not read; the schedule uses the default options, so its dates can differ from the file's",
            None,
        );
    }

    let mut plans = Vec::with_capacity(projects.len());
    for project in &projects {
        plans.push(xer_wbs::plan(&data, project.project, &mut notes)?);
    }
    let mut built = build(
        &projects,
        &plans,
        &mut schedule,
        &mut calendars,
        origin_day,
        PercentColumns::of(&parsed),
        &mut notes,
    )?;
    xer_codes::attach(&data, &built.task_info, &mut built.imported, &mut notes);
    note_actuals_after_data_date(&built.imported, data_date, &mut notes);
    xer_links::build(&data, &projects, &built, &mut schedule, &mut notes)?;
    apply_project_start(&built, &mut schedule)?;
    Ok(Imported {
        schedule,
        tasks: built.imported,
        unsupported: notes.finish(),
        data_date,
        origin_day,
        project_start: None,
    })
}

fn check_ids(data: &xer::Data) -> Result<(), ImportError> {
    let mut seen = HashSet::with_capacity(data.project_rows.len());
    for project in &data.project_rows {
        if !seen.insert(project.proj_id) {
            return Err(ImportError::DuplicateId {
                table: "PROJECT".to_owned(),
                id: project.proj_id,
            });
        }
    }
    seen.clear();
    for task in &data.task_rows {
        if !seen.insert(task.task_id) {
            return Err(ImportError::DuplicateId {
                table: "TASK".to_owned(),
                id: task.task_id,
            });
        }
    }
    seen.clear();
    for wbs in &data.wbs_rows {
        if !seen.insert(wbs.wbs_id) {
            return Err(ImportError::DuplicateId {
                table: "PROJWBS".to_owned(),
                id: wbs.wbs_id,
            });
        }
    }
    Ok(())
}

fn check_nonfinite_durations(parsed: &xer::Xer<'_>) -> Result<(), ImportError> {
    let Some(table) = parsed.table("TASK") else {
        return Ok(());
    };
    for field in ["target_drtn_hr_cnt", "remain_drtn_hr_cnt"] {
        let Some(column) = table.column(field) else {
            continue;
        };
        for row in table.rows() {
            let Some(value) = row.cell(column) else {
                continue;
            };
            let value = value.trim();
            if value.parse::<f64>().is_ok_and(|hours| !hours.is_finite()) {
                return Err(ImportError::Invalid {
                    element: field.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_nonfinite_lags(parsed: &xer::Xer<'_>) -> Result<(), ImportError> {
    let Some(table) = parsed.table("TASKPRED") else {
        return Ok(());
    };
    let Some(column) = table.column("lag_hr_cnt") else {
        return Ok(());
    };
    for row in table.rows() {
        let Some(value) = row.cell(column) else {
            continue;
        };
        let value = value.trim();
        if value.parse::<f64>().is_ok_and(|hours| !hours.is_finite()) {
            return Err(ImportError::Invalid {
                element: "lag_hr_cnt".to_owned(),
                value: value.to_owned(),
            });
        }
    }
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "used as `map_err(xer_error)`, which hands the error over by value; a `&` parameter would not fit"
)]
fn xer_error(source: xer::Error) -> ImportError {
    ImportError::Xer {
        message: source.to_string(),
    }
}

fn origin_day(projects: &[&ProjectRow], notes: &mut Notes) -> Result<i64, ImportError> {
    let Some(minutes) = projects
        .iter()
        .filter_map(|project| project.plan_start_date)
        .min()
    else {
        notes.add(
            Section::Project,
            "Project: the file has no start date, so the schedule starts on Monday 2000-01-03 and every date follows from that",
            None,
        );
        return Ok(days_from_civil(2000, 1, 3));
    };
    let day = minutes.div_euclid(1_440);
    day.checked_sub(i64::from(weekday_of(day)))
        .ok_or_else(|| ImportError::Invalid {
            element: "plan_start_date".to_owned(),
            value: minutes.to_string(),
        })
}

/// An instant from the origin Monday for a file date (minutes since 1970-01-01 00:00). Unlike
/// [`relative_instant`] it does not clamp: a date before the origin stays negative. `None` when
/// the arithmetic overflows.
pub(crate) fn instant_from_origin(minutes: i64, origin_day: i64) -> Option<i64> {
    origin_day
        .checked_mul(1_440)
        .and_then(|origin| minutes.checked_sub(origin))
}

/// The data date of the selected projects: the latest `last_recalc_date`, as an instant from the
/// origin. A date before the origin is dropped and noted; differing dates are each named.
fn data_date(projects: &[&ProjectRow], origin_day: i64, notes: &mut Notes) -> Option<i64> {
    let mut dates: Vec<(u32, i64)> = Vec::new();
    for project in projects {
        let Some(minutes) = project.last_recalc_date else {
            continue;
        };
        match instant_from_origin(minutes, origin_day) {
            Some(instant) if instant >= 0 => {
                let id = project.proj_id.to_string();
                if range::data_date(instant, Some(&id), notes).is_some() {
                    dates.push((project.proj_id, minutes));
                }
            }
            _ => notes.add(
                Section::Project,
                "The data date is before the schedule start and was not read; progress is measured without it, so dates can differ from the file's",
                Some(&project.proj_id.to_string()),
            ),
        }
    }
    let latest = dates.iter().map(|(_, minutes)| *minutes).max()?;
    if dates.iter().any(|(_, minutes)| *minutes != latest) {
        for (proj_id, minutes) in &dates {
            notes.add(
                Section::Project,
                "Projects have different data dates; the latest is used for all of them, so dates can differ from the file's",
                Some(&format!("{proj_id} {}", format_minutes(*minutes))),
            );
        }
    }
    instant_from_origin(latest, origin_day)
}

fn relative_instant(minutes: i64, origin_day: i64) -> Result<i64, ImportError> {
    origin_day
        .checked_mul(1_440)
        .and_then(|origin| minutes.checked_sub(origin))
        .map(|instant| instant.max(0))
        .ok_or_else(|| ImportError::Invalid {
            element: "plan_start_date".to_owned(),
            value: minutes.to_string(),
        })
}
