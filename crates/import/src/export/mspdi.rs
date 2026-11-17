//! Write an imported project as an MS Project XML (MSPDI) file: the inverse of `parse_mspdi`.
//!
//! The writer holds only what the project holds. It writes the project start, the status date,
//! the calendars, and for each task its name, outline level, `Summary`, duration, milestone,
//! start-no-earlier-than, calendar, WBS code, percent complete, the file's own start and finish
//! and its baseline, with each link's type and lag. Everything else comes back in
//! [`Exported::left_out`]. Reading the written file gives the same schedule, tasks, links,
//! calendars and dates (`crates/import/tests/mspdi_export.rs`). That MS Project itself opens the
//! file is not tested here: a person with a copy has to check it.

use std::collections::HashSet;

use crate::{ImportError, Imported};
use tasks::write_tasks;
use xml::{Xml, stamp};

mod calendars;
mod constraint;
mod tasks;
mod xml;

/// A written MS Project XML file and what it leaves out.
#[derive(Debug, Clone)]
pub struct Exported {
    /// The UTF-8 bytes of the file.
    pub bytes: Vec<u8>,
    /// One line for each thing the project holds that the file does not carry, and for each
    /// thing the reader had already left out of the project when it was imported.
    pub left_out: Vec<String>,
}

/// What the task writer needs to know about the project, worked out once.
pub(super) struct Plan<'a> {
    pub(super) imported: &'a Imported,
    /// Indexes of the live tasks, in file order.
    pub(super) live: Vec<usize>,
    /// The `UID` written for each task, by task index.
    pub(super) uids: Vec<u32>,
    /// Whether each task is written as a summary, by task index.
    pub(super) summary: Vec<bool>,
    /// The calendar index every summary uses and the project names as its default.
    pub(super) default_calendar: usize,
}

impl<'a> Plan<'a> {
    fn new(imported: &'a Imported, left_out: &mut Vec<String>) -> Plan<'a> {
        let schedule = &imported.schedule;
        let count = schedule.tasks().len().min(imported.tasks.len());
        let live: Vec<usize> = (0..count)
            .filter(|&index| {
                schedule
                    .tasks()
                    .get(index)
                    .is_some_and(|task| !task.deleted)
            })
            .collect();
        let mut seen = HashSet::new();
        let unique = live
            .iter()
            .all(|&index| seen.insert(imported.tasks.get(index).map(|task| task.uid)));
        let mut uids = vec![0; count];
        for (position, &index) in live.iter().enumerate() {
            let source = imported.tasks.get(index).map_or(0, |task| task.uid);
            if let Some(uid) = uids.get_mut(index) {
                *uid = if unique {
                    source
                } else {
                    u32::try_from(position + 1).unwrap_or(u32::MAX)
                };
            }
        }
        if !unique {
            left_out
                .push("task UIDs: the source repeats them, so tasks are numbered from 1".into());
        }
        let mut plan = Plan {
            imported,
            live,
            uids,
            summary: vec![false; count],
            default_calendar: 0,
        };
        plan.pick_summaries(left_out);
        plan
    }

    fn has_children(&self, position: usize) -> bool {
        let level = |position: usize| {
            self.live
                .get(position)
                .and_then(|&index| self.imported.tasks.get(index))
                .map(|task| task.outline_level)
        };
        matches!((level(position), level(position + 1)), (Some(own), Some(next)) if next > own)
    }

    /// A task with sub-tasks is a summary when it holds no work of its own, which is all the
    /// reader keeps of one: no duration and the default calendar. The default is the calendar of
    /// the first such task. A parent that does hold its own work, or the file's own start or
    /// finish (the reader drops a summary's), is written as a task.
    fn pick_summaries(&mut self, left_out: &mut Vec<String>) {
        let schedule = &self.imported.schedule;
        let parents: Vec<usize> = self
            .live
            .iter()
            .enumerate()
            .filter(|&(position, _)| self.has_children(position))
            .map(|(_, &index)| index)
            .collect();
        let idle = |index: usize| {
            schedule
                .tasks()
                .get(index)
                .is_some_and(|task| task.duration == 0)
        };
        let dated = |index: usize| {
            self.imported
                .tasks
                .get(index)
                .is_some_and(|task| task.file_start.is_some() || task.file_finish.is_some())
        };
        let calendar = |index: usize| {
            schedule
                .tasks()
                .get(index)
                .map_or(0, |task| task.calendar.0 as usize)
        };
        let plain = |index: usize| idle(index) && !dated(index);
        if let Some(&first) = parents.iter().find(|&&index| plain(index)) {
            self.default_calendar = calendar(first);
        }
        for index in parents {
            if plain(index) && calendar(index) == self.default_calendar {
                if let Some(summary) = self.summary.get_mut(index) {
                    *summary = true;
                }
            } else {
                let uid = self.uids.get(index).copied().unwrap_or(0);
                left_out.push(format!(
                    "task {uid}: has sub-tasks but holds its own duration, calendar or dates, written as a task, not a summary"
                ));
            }
        }
    }
}

/// Lines for the project's own facts the file cannot state.
fn project_notes(imported: &Imported, plan: &Plan<'_>, left_out: &mut Vec<String>) {
    let schedule = &imported.schedule;
    if imported.project_start.is_none() {
        left_out.push(
            "project start: the source states none, so StartDate is the schedule origin (the Monday at 00:00) and every task starts no earlier than it"
                .into(),
        );
    }
    let deleted = schedule.tasks().len() - schedule.live_task_count();
    let removed = schedule
        .dependencies()
        .iter()
        .enumerate()
        .filter(|&(index, _)| {
            u32::try_from(index).is_ok_and(|id| !schedule.is_dependency_live(engine::DepId(id)))
        })
        .count();
    if deleted + removed > 0 {
        left_out.push(format!(
            "{deleted} deleted task(s) and {removed} removed link(s) are not written"
        ));
    }
    let coded = plan
        .live
        .iter()
        .filter_map(|&index| imported.tasks.get(index))
        .filter(|task| task.code.is_some() || !task.activity_codes.is_empty())
        .count();
    if coded > 0 {
        left_out.push(format!(
            "P6 activity ids and activity codes on {coded} task(s) are not written"
        ));
    }
}

/// Write `imported` as an MS Project XML file.
///
/// # Errors
///
/// [`ImportError::Invalid`] when a date or duration cannot be written in the file's form: a year
/// outside 0000 to 9999 or a negative duration.
pub fn export_mspdi(imported: &Imported) -> Result<Exported, ImportError> {
    let mut left_out = Vec::new();
    let plan = Plan::new(imported, &mut left_out);
    project_notes(imported, &plan, &mut left_out);

    let mut xml = Xml::new();
    xml.open_project();
    xml.leaf(
        "StartDate",
        &stamp(
            imported.origin_day,
            imported.project_start.unwrap_or(0),
            "StartDate",
        )?,
    );
    if !imported.schedule.calendars().is_empty() {
        xml.leaf("CalendarUID", &(plan.default_calendar + 1).to_string());
    }
    if let Some(data_date) = imported.data_date {
        xml.leaf(
            "StatusDate",
            &stamp(imported.origin_day, data_date, "StatusDate")?,
        );
    }
    calendars::write(&mut xml, &imported.schedule, imported.origin_day)?;
    write_tasks(&mut xml, &plan, &mut left_out)?;
    xml.close("Project");

    left_out.extend(
        (imported.unsupported.iter())
            .map(|line| format!("not in the project, left out by the reader: {line}")),
    );
    Ok(Exported {
        bytes: xml.finish(),
        left_out,
    })
}
