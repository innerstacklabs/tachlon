//! The `TASKPRED` and `SCHEDOPTIONS` tables of an XER file: each live link between two activities
//! and the schedule's lag calendar, written once for every project.
//!
//! A link is written when both ends are live activities. A link with a deleted end is dead and
//! has no row; a link to a summary has no row either, because a summary is a WBS node in the
//! file, not a `TASK` row. The task ids are the ones the `TASK` table gets: the position among
//! the live activities, from 1.

use engine::{LagCalendar, LinkType, Schedule};
use xer::{Cell, Writer};

use super::rows::cell_error;
use super::tasks::Project;
use crate::{ImportError, Imported};

const PRED_FIELDS: [&str; 5] = [
    "task_pred_id",
    "task_id",
    "pred_task_id",
    "pred_type",
    "lag_hr_cnt",
];
const OPTION_FIELDS: [&str; 2] = ["proj_id", "sched_calendar_on_relationship_lag"];

/// What a schedule task is in the file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    /// A `TASK` row with this `task_id`.
    Activity(usize),
    /// A WBS node: a live task whose next live task has a deeper outline level.
    Summary,
    /// A deleted task, or one past the end of `Imported::tasks`, which the writer does not lay out.
    Absent,
}

/// For each schedule task, what it is in the file. The ids are the ones the `TASK` table gets.
fn ends(imported: &Imported) -> Vec<End> {
    let schedule = &imported.schedule;
    let count = schedule.tasks().len().min(imported.tasks.len());
    let live: Vec<usize> = (0..count)
        .filter(|&index| !schedule.tasks()[index].deleted)
        .collect();
    let mut ends = vec![End::Absent; schedule.tasks().len()];
    let mut next = 1;
    for (position, &index) in live.iter().enumerate() {
        let level = imported.tasks[index].outline_level;
        let summary = live
            .get(position + 1)
            .is_some_and(|&after| imported.tasks[after].outline_level > level);
        ends[index] = if summary {
            End::Summary
        } else {
            next += 1;
            End::Activity(next - 1)
        };
    }
    ends
}

fn link_code(kind: LinkType) -> &'static str {
    match kind {
        LinkType::FS => "PR_FS",
        LinkType::SS => "PR_SS",
        LinkType::FF => "PR_FF",
        LinkType::SF => "PR_SF",
    }
}

/// The `sched_calendar_on_relationship_lag` value, and whether the file can say it for `project`.
fn lag_value(schedule: &Schedule, project: &Project) -> (&'static str, bool) {
    match schedule.lag_calendar() {
        LagCalendar::Successor => ("rcal_Successor", true),
        LagCalendar::Predecessor => ("rcal_Predecessor", true),
        LagCalendar::TwentyFourHour => ("rcal_24Hour", true),
        LagCalendar::Fixed(id) => (
            "rcal_ProjDefault",
            project.calendar == Some(id.0 as usize + 1),
        ),
    }
}

/// Write the `TASKPRED` table (a link's type and its lag in hours) and the `SCHEDOPTIONS` table
/// (one row per project, holding the lag calendar). What the file cannot carry goes to
/// `left_out`: a link to a summary, a link with a lag calendar of its own, and a fixed lag
/// calendar that is not a project's own.
pub(super) fn write_links(
    writer: &mut Writer,
    imported: &Imported,
    projects: &[Project],
    left_out: &mut Vec<String>,
) -> Result<(), ImportError> {
    let schedule = &imported.schedule;
    let ends = ends(imported);
    let end = |id: engine::TaskId| ends.get(id.0 as usize).copied().unwrap_or(End::Absent);
    let error = |error| cell_error("TASKPRED", error);
    writer.table("TASKPRED", &PRED_FIELDS).map_err(error)?;
    let mut row = 0;
    for (dep, link) in schedule.live_dependencies() {
        let (End::Activity(pred), End::Activity(succ)) = (end(link.pred), end(link.succ)) else {
            let why = if end(link.pred) == End::Absent || end(link.succ) == End::Absent {
                "an end has no row in the file"
            } else {
                "an end is a summary"
            };
            left_out.push(format!(
                "link from task {} to task {}: {why}, the link is not written",
                link.pred.0, link.succ.0
            ));
            continue;
        };
        if schedule.dependency_lag_calendar(dep).is_some() {
            left_out.push(format!(
                "link from task {} to task {}: its own lag calendar is not written, the schedule's is used",
                link.pred.0, link.succ.0
            ));
        }
        row += 1;
        #[expect(
            clippy::cast_precision_loss,
            reason = "the engine bounds a lag at MAX_LAG (52,704,000 minutes), far below 2^53, so the f64 cast is exact"
        )]
        let hours = (link.lag as f64 / 60.0).to_string();
        writer
            .row(&[
                Cell::Text(&row.to_string()),
                Cell::Text(&succ.to_string()),
                Cell::Text(&pred.to_string()),
                Cell::Text(link_code(link.kind)),
                Cell::Text(&hours),
            ])
            .map_err(error)?;
    }

    let error = |error| cell_error("SCHEDOPTIONS", error);
    writer
        .table("SCHEDOPTIONS", &OPTION_FIELDS)
        .map_err(error)?;
    for (index, project) in projects.iter().enumerate() {
        let (value, exact) = lag_value(schedule, project);
        if !exact {
            left_out.push(format!(
                "project {}: the lag calendar is a calendar other than the project's own, written as the project's",
                index + 1
            ));
        }
        writer
            .row(&[Cell::Text(&(index + 1).to_string()), Cell::Text(value)])
            .map_err(error)?;
    }
    Ok(())
}
