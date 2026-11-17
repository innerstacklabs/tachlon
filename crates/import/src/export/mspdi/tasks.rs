//! The `Tasks` element: one `Task` per live task in outline order, each with its links.

use engine::{DepId, Dependency, LagCalendar, Limit, LinkType, Schedule};

use super::Plan;
use super::constraint::{Written, choose};
use super::xml::{Xml, clean, stamp};
use crate::fields::invalid;
use crate::{ImportError, ImportedTask};

/// `DurationFormat` and `LagFormat` 5 is hours: a count of minutes needs no day length. 6 is
/// elapsed hours, which the reader counts in all minutes.
const HOURS: &str = "5";
const ELAPSED_HOURS: &str = "6";

fn duration_text(minutes: i64) -> Result<String, ImportError> {
    if minutes < 0 {
        return Err(invalid("Duration", &minutes.to_string()));
    }
    Ok(format!("PT{}H{}M0S", minutes / 60, minutes % 60))
}

fn link_type_code(kind: LinkType) -> &'static str {
    match kind {
        LinkType::FF => "0",
        LinkType::FS => "1",
        LinkType::SF => "2",
        LinkType::SS => "3",
    }
}

/// The link's lag as MSPDI tenths of a minute and the format code. The reader counts a lag in an
/// elapsed unit in all minutes and any other in the successor's working time, so those are the two
/// the file can say; a lag counted in anything else is written as the successor's and `foreign`
/// is raised.
fn lag_of(
    schedule: &Schedule,
    index: usize,
    dep: &Dependency,
    foreign: &mut usize,
) -> Result<(String, &'static str), ImportError> {
    let own = u32::try_from(index)
        .ok()
        .and_then(|id| schedule.dependency_lag_calendar(DepId(id)));
    let tenths = dep
        .lag
        .checked_mul(10)
        .ok_or_else(|| invalid("LinkLag", &dep.lag.to_string()))?;
    let format = match own.unwrap_or_else(|| schedule.lag_calendar()) {
        LagCalendar::TwentyFourHour if dep.lag != 0 => ELAPSED_HOURS,
        LagCalendar::Predecessor | LagCalendar::Fixed(_) => {
            *foreign += 1;
            HOURS
        }
        _ => HOURS,
    };
    Ok((tenths.to_string(), format))
}

fn write_links(
    xml: &mut Xml,
    plan: &Plan<'_>,
    links: &[usize],
    foreign: &mut usize,
) -> Result<(), ImportError> {
    let schedule = &plan.imported.schedule;
    for &index in links {
        let Some(dep) = schedule.dependencies().get(index) else {
            continue;
        };
        let (lag, format) = lag_of(schedule, index, dep, foreign)?;
        xml.open("PredecessorLink");
        xml.leaf(
            "PredecessorUID",
            &plan
                .uids
                .get(dep.pred.0 as usize)
                .copied()
                .unwrap_or(0)
                .to_string(),
        );
        xml.leaf("Type", link_type_code(dep.kind));
        xml.leaf("LinkLag", &lag);
        xml.leaf("LagFormat", format);
        xml.close("PredecessorLink");
    }
    Ok(())
}

fn write_baseline(xml: &mut Xml, meta: &ImportedTask, origin_day: i64) -> Result<(), ImportError> {
    if meta.baseline_start.is_none() && meta.baseline_finish.is_none() {
        return Ok(());
    }
    xml.open("Baseline");
    xml.leaf("Number", "0");
    if let Some(start) = meta.baseline_start {
        xml.leaf("Start", &stamp(origin_day, start, "Start")?);
    }
    if let Some(finish) = meta.baseline_finish {
        xml.leaf("Finish", &stamp(origin_day, finish, "Finish")?);
    }
    xml.close("Baseline");
    Ok(())
}

/// `ActualStart`, then `ActualFinish` of a finished task and `RemainingDuration`. The reader takes
/// the finish and the remaining work only with a start, so a task with none writes none of the
/// three (`task_notes` lists it).
fn write_actuals(xml: &mut Xml, meta: &ImportedTask, origin_day: i64) -> Result<(), ImportError> {
    let Some(start) = meta.actual_start else {
        return Ok(());
    };
    xml.leaf("ActualStart", &stamp(origin_day, start, "ActualStart")?);
    if let Some(finish) = meta.actual_finish {
        xml.leaf("ActualFinish", &stamp(origin_day, finish, "ActualFinish")?);
    }
    if let Some(remaining) = meta.remaining {
        xml.leaf("RemainingDuration", &duration_text(remaining)?);
    }
    Ok(())
}

/// The task's own start-no-earlier-than and its limits. The project start is written as
/// `StartDate` and the reader raises an earlier constraint to it, so only a later one is the
/// task's own. A summary has neither.
fn own_constraints(plan: &Plan<'_>, index: usize) -> (Option<i64>, Limit) {
    let imported = plan.imported;
    let summary = plan.summary.get(index).copied().unwrap_or(false);
    let Some(task) = imported.schedule.tasks().get(index).filter(|_| !summary) else {
        return (None, Limit::default());
    };
    let snet = task
        .snet
        .filter(|snet| imported.project_start.is_none_or(|start| *snet > start));
    (snet, task.limit)
}

/// Lines for one task's facts the file cannot state.
fn task_notes(plan: &Plan<'_>, index: usize, uid: u32, left_out: &mut Vec<String>) {
    let imported = plan.imported;
    let (Some(task), Some(meta)) = (
        imported.schedule.tasks().get(index),
        imported.tasks.get(index),
    ) else {
        return;
    };
    if meta.actual_start.is_none() {
        if meta.in_progress {
            left_out.push(format!(
                "task {uid}: in progress with no actual start, so it reads back not started"
            ));
        }
        if meta.actual_finish.is_some() {
            left_out.push(format!(
                "task {uid}: an actual finish with no actual start is not written"
            ));
        }
    }
    let summary = plan.summary.get(index).copied().unwrap_or(false);
    if summary && task.snet != imported.project_start {
        left_out.push(format!(
            "task {uid}: a summary's own start-no-earlier-than is not written"
        ));
    }
    if let (false, Some(snet), Some(start)) = (summary, task.snet, imported.project_start)
        && snet < start
    {
        left_out.push(format!(
            "task {uid}: its start-no-earlier-than is earlier than the project start, so it is not written and reads back at the project start"
        ));
    }
    let (snet, limit) = own_constraints(plan, index);
    if let (Some(kept), dropped) = choose(snet, &limit)
        && !dropped.is_empty()
    {
        left_out.push(format!(
            "task {uid}: its {} constraint is not written (the file carries one constraint per task, the {}), so it reads back without it",
            dropped.join(" and "),
            kept.name
        ));
    }
    if !summary && task.snet.is_none() && imported.project_start.is_some() {
        left_out.push(format!(
            "task {uid}: has no start constraint, and reads back no earlier than the project start"
        ));
    }
}

fn write_task(
    xml: &mut Xml,
    plan: &Plan<'_>,
    position: usize,
    index: usize,
    links: &[usize],
    left_out: &mut Vec<String>,
) -> Result<(), ImportError> {
    let imported = plan.imported;
    let origin_day = imported.origin_day;
    let (Some(task), Some(meta)) = (
        imported.schedule.tasks().get(index),
        imported.tasks.get(index),
    ) else {
        return Ok(());
    };
    let uid = plan.uids.get(index).copied().unwrap_or(0);
    let summary = plan.summary.get(index).copied().unwrap_or(false);
    task_notes(plan, index, uid, left_out);

    xml.open("Task");
    xml.leaf("UID", &uid.to_string());
    xml.leaf("ID", &(position + 1).to_string());
    let (name, changed) = clean(&meta.name);
    if changed {
        left_out.push(format!(
            "task {uid}: characters XML cannot hold were removed from the name"
        ));
    }
    xml.leaf("Name", &name);
    if let Some(code) = &meta.wbs_code {
        xml.leaf("WBS", &clean(code).0);
    }
    xml.leaf("OutlineLevel", &meta.outline_level.to_string());
    if let Some(start) = meta.file_start {
        xml.leaf("Start", &stamp(origin_day, start, "Start")?);
    }
    if let Some(finish) = meta.file_finish {
        xml.leaf("Finish", &stamp(origin_day, finish, "Finish")?);
    }
    xml.leaf("Duration", &duration_text(task.duration)?);
    xml.leaf("DurationFormat", HOURS);
    if let Some(percent) = meta.progress {
        xml.leaf("PercentComplete", &percent.to_string());
    }
    xml.leaf(
        "Milestone",
        if task.duration == 0 && !summary {
            "1"
        } else {
            "0"
        },
    );
    xml.leaf("Summary", if summary { "1" } else { "0" });
    if !summary {
        write_actuals(xml, meta, origin_day)?;
    }
    let (snet, limit) = own_constraints(plan, index);
    let (constraint, _) = choose(snet, &limit);
    if let Some(Written { code, .. }) = &constraint {
        xml.leaf("ConstraintType", code);
    }
    if !summary {
        xml.leaf("CalendarUID", &(task.calendar.0 as usize + 1).to_string());
    }
    if let Some(Written { at: Some(at), .. }) = constraint {
        xml.leaf("ConstraintDate", &stamp(origin_day, at, "ConstraintDate")?);
    }
    if let Some(deadline) = meta.deadline {
        xml.leaf("Deadline", &stamp(origin_day, deadline, "Deadline")?);
    }
    let mut foreign = 0;
    write_links(xml, plan, links, &mut foreign)?;
    if foreign > 0 {
        left_out.push(format!(
            "task {uid}: {foreign} link lag(s) counted in another calendar are written as the successor's"
        ));
    }
    write_baseline(xml, meta, origin_day)?;
    xml.close("Task");
    Ok(())
}

pub(super) fn write_tasks(
    xml: &mut Xml,
    plan: &Plan<'_>,
    left_out: &mut Vec<String>,
) -> Result<(), ImportError> {
    let schedule = &plan.imported.schedule;
    let mut links: Vec<Vec<usize>> = vec![Vec::new(); schedule.tasks().len()];
    for (index, dep) in schedule.dependencies().iter().enumerate() {
        let live = u32::try_from(index).is_ok_and(|id| schedule.is_dependency_live(DepId(id)));
        if let (true, Some(own)) = (live, links.get_mut(dep.succ.0 as usize)) {
            own.push(index);
        }
    }
    xml.open("Tasks");
    for (position, &index) in plan.live.iter().enumerate() {
        let own = links.get(index).map_or(&[][..], Vec::as_slice);
        write_task(xml, plan, position, index, own, left_out)?;
    }
    xml.close("Tasks");
    Ok(())
}
