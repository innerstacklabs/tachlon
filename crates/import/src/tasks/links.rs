use std::collections::HashMap;

use engine::{Dependency, LagCalendar, LinkType, Schedule, TaskId};

use crate::ImportError;
use crate::dom::Node;
use crate::fields::{
    elapsed_format, format_code, invalid, optional_i64, optional_u32, percent_format, required_u32,
};
use crate::notes::Notes;
use crate::range;

fn link_type(node: &Node) -> Result<LinkType, ImportError> {
    match optional_u32(node, "Type")?.unwrap_or(1) {
        0 => Ok(LinkType::FF),
        1 => Ok(LinkType::FS),
        2 => Ok(LinkType::SF),
        3 => Ok(LinkType::SS),
        value => Err(invalid("Type", &value.to_string())),
    }
}

/// What a link states besides its predecessor: the lag in minutes, whether those minutes are
/// elapsed time (counted in the 24-hour lag calendar) rather than working time, the type, and
/// which notes the lag earns.
struct Terms {
    lag: i64,
    elapsed: bool,
    kind: LinkType,
    fraction: bool,
    percent: bool,
}

fn terms(node: &Node) -> Result<Terms, ImportError> {
    let tenths = optional_i64(node, "LinkLag")?.unwrap_or(0);
    let mut lag = tenths / 10;
    let mut elapsed = false;
    let mut percent = false;
    if let Some((_, normalized)) = format_code(node, "LagFormat")? {
        if percent_format(normalized) {
            lag = 0;
            percent = true;
        } else {
            elapsed = elapsed_format(normalized) && lag != 0;
        }
    }
    Ok(Terms {
        lag,
        elapsed,
        kind: link_type(node)?,
        fraction: tenths % 10 != 0,
        percent,
    })
}

/// One `PredecessorLink`, read when its task is and added to the schedule once every task is in:
/// the predecessor may come later in the file. Each part keeps its error so the first one the
/// links raise is the one a whole-file reader would, and `terms` is not looked at when the
/// predecessor is not in the file.
pub(super) struct Link {
    succ: usize,
    predecessor: Result<u32, Box<ImportError>>,
    terms: Result<Terms, Box<ImportError>>,
}

/// The links of the task at position `succ`, in document order.
pub(super) fn read_links(task: &Node, succ: usize, links: &mut Vec<Link>) {
    for link in task.children_named("PredecessorLink") {
        links.push(Link {
            succ,
            predecessor: required_u32(link, "PredecessorUID").map_err(Box::new),
            terms: terms(link).map_err(Box::new),
        });
    }
}

/// Add every link to `schedule`. `ids` and `uids` are the imported tasks' engine ids and UIDs, in
/// order, and `by_uid` maps a UID to its position.
pub(super) fn add_links(
    links: Vec<Link>,
    ids: &[TaskId],
    uids: &[u32],
    by_uid: &HashMap<u32, usize>,
    schedule: &mut Schedule,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    for link in links {
        let (Some(&succ_task), Some(&succ_uid)) = (ids.get(link.succ), uids.get(link.succ)) else {
            continue;
        };
        let predecessor_uid = link.predecessor.map_err(|error| *error)?;
        let Some(&predecessor) = by_uid.get(&predecessor_uid).and_then(|at| ids.get(*at)) else {
            notes.line(format!(
                "Task {succ_uid}: a link from task {predecessor_uid} was dropped because that task is not in the file; the task is not held back by it, so its dates can differ from the file's."
            ));
            continue;
        };
        let terms = link.terms.map_err(|error| *error)?;
        if terms.fraction {
            notes.line(format!(
                "Task {succ_uid}: a link lag has a fraction of a minute that was not read; the lag is rounded down to whole minutes, so dates can differ by under a minute."
            ));
        }
        let lag = range::lag(terms.lag, &succ_uid.to_string(), notes);
        if terms.percent {
            notes.line(format!(
                "Task {succ_uid}: a link lag given as a percent was not read; the link has no lag, so dates can differ from the file's."
            ));
        }
        let dep = schedule.add_dependency(Dependency {
            pred: predecessor,
            succ: succ_task,
            kind: terms.kind,
            lag,
        })?;
        if terms.elapsed && lag != 0 {
            schedule.set_dependency_lag_calendar(dep, Some(LagCalendar::TwentyFourHour))?;
        }
    }
    Ok(())
}
