//! The header line and the `PROJECT` row of an XER file.

use xer::{Cell, WriteError, Writer};

use super::tasks::Project;
use crate::activity_id::next_activity_id;
use crate::fields::invalid;
use crate::units::civil_from_days;
use crate::{ImportError, Imported};

/// The XER version the file claims.
const VERSION: &str = "19.12";

const PROJECT_FIELDS: [&str; 5] = [
    "proj_id",
    "proj_short_name",
    "clndr_id",
    "plan_start_date",
    "last_recalc_date",
];

/// An instant from the schedule origin as the date the reader parses.
pub(super) fn absolute(origin_day: i64, instant: i64) -> Result<String, ImportError> {
    let minutes = origin_day
        .checked_mul(1_440)
        .and_then(|origin| origin.checked_add(instant))
        .ok_or_else(|| invalid("export date", &instant.to_string()))?;
    date_time(minutes)
}

/// Minutes since 1970-01-01 as the `YYYY-MM-DD HH:MM` the reader parses.
fn date_time(minutes: i64) -> Result<String, ImportError> {
    let (year, month, day) = civil_from_days(minutes.div_euclid(1_440));
    if !(1..=9999).contains(&year) {
        return Err(invalid("export date", &minutes.to_string()));
    }
    let minute = minutes.rem_euclid(1_440);
    Ok(format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minute / 60,
        minute % 60
    ))
}

pub(super) fn cell_error(table: &str, error: WriteError) -> ImportError {
    invalid(table, &error.to_string())
}

/// Write the `ERMHDR` line: the version, the export time and the fixed fields P6 expects.
pub(super) fn write_header(writer: &mut Writer, exported_at: i64) -> Result<(), ImportError> {
    let stamp = date_time(exported_at)?;
    writer
        .header(&[
            Cell::Text(VERSION),
            Cell::Text(&stamp),
            Cell::Text("Project"),
            Cell::Text("admin"),
            Cell::Text("admin"),
            Cell::Text("db"),
            Cell::Text("Project Management"),
            Cell::Text("USD"),
        ])
        .map_err(|error| cell_error("ERMHDR", error))
}

/// Write the `PROJECT` table: one row per project, numbered from 1. A project's `calendar` is the
/// `clndr_id` of its calendar, empty when the schedule has none. `plan_start_date` is the
/// project's start and `last_recalc_date` the data date, empty when the project has none.
pub(super) fn write_projects(
    writer: &mut Writer,
    projects: &[Project],
    origin_day: i64,
    data_date: Option<i64>,
) -> Result<(), ImportError> {
    let error = |error| cell_error("PROJECT", error);
    writer.table("PROJECT", &PROJECT_FIELDS).map_err(error)?;
    let recalc = data_date
        .map(|instant| absolute(origin_day, instant))
        .transpose()?
        .unwrap_or_default();
    for (index, project) in projects.iter().enumerate() {
        let start = absolute(origin_day, project.start)?;
        let id = (index + 1).to_string();
        let calendar = project.calendar.map_or_default(|id| id.to_string());
        writer
            .row(&[
                Cell::Text(&id),
                Cell::Text(&project.name),
                Cell::Text(&calendar),
                Cell::Text(&start),
                Cell::Text(&recalc),
            ])
            .map_err(error)?;
    }
    Ok(())
}

/// `imported` with an activity ID on every leaf that has none, one no source task holds. The
/// writer would give such a leaf an ID that could be a dropped source task's, and the leaf would
/// take that task's id; here the IDs are chosen with every source ID taken (decision 0009, item 4).
pub(super) fn with_codes(imported: &Imported, source: &[u8]) -> Result<Imported, ImportError> {
    let source = xer::parse(source).map_err(|error| invalid("source", &error.to_string()))?;
    let mut taken: Vec<String> = Vec::new();
    if let Some(tasks) = source.table("TASK")
        && let Some(column) = tasks.column("task_code")
    {
        let codes = tasks.rows().iter().filter_map(|row| row.cell(column));
        taken.extend(codes.map(std::borrow::Cow::into_owned));
    }
    let schedule = &imported.schedule;
    let count = schedule.tasks().len().min(imported.tasks.len());
    let live: Vec<usize> = (0..count)
        .filter(|&i| !schedule.tasks()[i].deleted)
        .collect();
    taken.extend(live.iter().filter_map(|&i| imported.tasks[i].code.clone()));
    let mut out = imported.clone();
    let mut preceding: Option<String> = None;
    for (at, &i) in live.iter().enumerate() {
        let level = imported.tasks[i].outline_level;
        if live
            .get(at + 1)
            .is_some_and(|&n| imported.tasks[n].outline_level > level)
        {
            continue;
        }
        let code = if let Some(code) = imported.tasks[i]
            .code
            .as_deref()
            .filter(|c| !c.trim().is_empty())
        {
            code.to_owned()
        } else {
            let code = next_activity_id(preceding.as_deref(), taken.iter().map(String::as_str));
            taken.push(code.clone());
            out.tasks[i].code = Some(code.clone());
            code
        };
        preceding = Some(code);
    }
    Ok(out)
}
