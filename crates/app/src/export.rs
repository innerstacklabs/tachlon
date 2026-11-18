//! [`export_mspdi`] and [`export_xer`]: the session as an MS Project XML or a Primavera XER file,
//! edits included (gs#703).

use crate::Session;
use crate::document::{Extras, SourceFormat, live_order};
use engine::{DepId, Dependency, Instant, Schedule, ScheduleOptions, Task, TaskId};
use import::{Exported, ImportError, Imported, ImportedTask, XerLayout, XerOptions};
use std::collections::HashMap;

/// The session as a Primavera XER file: the live tasks in outline order as the XER writer lays
/// them out, edits included, a deleted task not written. The layout is [`XerLayout::FromXer`] when
/// the opened file (`extras.source`) was an XER, else [`XerLayout::One`] (one project named
/// `extras.name`). The export time in the header is fixed at 0, never the clock, so the same
/// session gives the same bytes. A started task keeps its status, actual dates and remaining work,
/// so the file reads back as the session has it. [`Exported::left_out`] lists what the XER writer
/// does not carry, and `extras.notes` come back there as what the reader had left out.
///
/// # Errors
///
/// [`ImportError::Invalid`] when a date cannot be written in the file's form or a cell holds a
/// character the file cannot carry.
pub fn export_xer(session: &Session, extras: &Extras<'_>) -> Result<Exported, ImportError> {
    let (mut imported, starts) = imported(session, extras, false);
    let from_xer = extras.source.is_some_and(|s| s.format == SourceFormat::Xer);
    if !from_xer {
        imported.project_start = starts
            .iter()
            .flatten()
            .copied()
            .chain(session.data_date())
            .min();
    }
    let options = XerOptions {
        exported_at: 0,
        name: extras.name.to_string(),
        layout: if from_xer {
            XerLayout::FromXer
        } else {
            XerLayout::One
        },
    };
    let mut exported = import::export_xer(&imported, &options)?;
    note_keys(session, &mut exported);
    Ok(exported)
}

/// The session as an MS Project XML file: the live tasks in outline order with their names,
/// durations, outline levels, links, calendars, progress, baseline and deadline, as edited. Each task's
/// `UID` is its id in the session plus one, so a deleted task leaves a gap and nothing is
/// renumbered. No task writes a start or finish date: MS Project calculates them from the
/// durations and links. A started task writes its actual start and remaining work, and a finished
/// one its actual finish, so the file reads back as the session has it.
///
/// The project start is the earliest start of any task. A task whose start the file would not
/// give back (its predecessors and the project start do not explain it) is written with its start
/// as a start-no-earlier-than, checked by reading the written file back and scheduling it. A start
/// that still differs is listed in [`Exported::left_out`], as are the activity ids and codes and the host keys the file cannot hold.
/// A baseline in the opened file (`extras.source`) that states a cost or one date but not both is
/// listed there too: the session keeps dates only. A task with both dates is in the session's
/// baseline and written back. `extras.notes` come back there as what the
/// reader had left out. The source file's own
/// unmodelled parts are not replayed.
///
/// # Errors
///
/// [`ImportError::Invalid`] when a date or duration cannot be written in the file's form.
pub fn export_mspdi(session: &Session, extras: &Extras<'_>) -> Result<Exported, ImportError> {
    let (mut imported, expected) = imported(session, extras, true);
    let mut exported = import::export_mspdi(&imported)?;
    let mut drift = drift(&exported.bytes, &expected);
    let earlier: Vec<usize> = drift
        .iter()
        .filter(|&&(at, read)| expected[at].is_some_and(|start| read < start))
        .map(|&(at, _)| at)
        .collect();
    if !earlier.is_empty() {
        for at in earlier {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let _ = imported.schedule.set_snet(TaskId(at as u32), expected[at]);
        }
        exported = import::export_mspdi(&imported)?;
        drift = self::drift(&exported.bytes, &expected);
    }
    for (at, _) in drift {
        let uid = imported.tasks[at].uid;
        exported.left_out.push(format!(
            "task {uid}: reads back with a different start than it has here"
        ));
    }
    let unwritten = source_baselines(extras)
        .map(|count| count.saturating_sub(dated_baselines(extras)))
        .filter(|&count| count > 0);
    if let Some(count) = unwritten {
        exported.left_out.push(format!(
            "the baseline saved in the opened file, on {count} {}, was not written (it states no dates)",
            if count == 1 { "task" } else { "tasks" }
        ));
    }
    note_keys(session, &mut exported);
    Ok(exported)
}

/// Lists the host keys of the live tasks, links and calendars in `exported.left_out`: neither file has a
/// place for them.
fn note_keys(session: &Session, exported: &mut Exported) {
    if session.schedule().options() != ScheduleOptions::default() {
        exported.left_out.push(
            "the schedule options are not written: the file reopens on the defaults".to_string(),
        );
    }
    if session.schedule().finish_by().is_some() {
        exported.left_out.push(
            "the must-finish-by date is not written: the file reopens with no date".to_string(),
        );
    }
    let (meta, keys) = (session.meta(), session.host_keys());
    let tasks = live_order(session)
        .iter()
        .filter(|task| meta[task.0 as usize].key.is_some())
        .count();
    let links = (session.schedule().live_dependencies())
        .filter(|(dep, _)| keys.link_key(*dep).is_some())
        .count();
    let counted = [
        (tasks, "task"),
        (links, "link"),
        (keys.calendar_count(), "calendar"),
    ];
    let named: Vec<String> = (counted.iter())
        .filter(|(count, _)| *count > 0)
        .map(|(count, kind)| format!("{count} {kind}(s)"))
        .collect();
    if !named.is_empty() {
        let named = named.join(", ");
        exported
            .left_out
            .push(format!("host keys on {named} are not written"));
    }
}

/// How many tasks of the opened MS Project file carry a saved Baseline 0, `None` when there is no
/// source file, it is not one, or none does. Dates or not. The file is read once more, as the
/// importer reads it: by element, under `Project/Tasks/Task/Baseline/Number`, whatever the
/// namespace prefix, attributes or spacing, and with comments, processing instructions and
/// CDATA kept out of the markup.
fn source_baselines(extras: &Extras<'_>) -> Option<usize> {
    let source = extras.source.filter(|s| s.format == SourceFormat::Mspdi)?;
    let text = String::from_utf8_lossy(source.bytes);
    let mut rest: &str = &text;
    let (mut path, mut number) = (Vec::<&str>::new(), String::new());
    let (mut zero, mut count) = (false, 0);
    let at_number =
        |path: &[&str]| path.len() == 5 && path[1..] == ["Tasks", "Task", "Baseline", "Number"];
    while let Some(open) = rest.find('<') {
        if at_number(&path) {
            number.push_str(&rest[..open]);
        }
        rest = &rest[open..];
        let skip = |end: &str| rest.find(end).map(|at| at + end.len());
        if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>")?;
            if at_number(&path) {
                number.push_str(&rest["<![CDATA[".len()..end]);
            }
            rest = &rest[end + 3..];
        } else if rest.starts_with("<!--") {
            rest = &rest[skip("-->")?..];
        } else if rest.starts_with("<?") {
            rest = &rest[skip("?>")?..];
        } else if rest.starts_with("<!") {
            rest = &rest[skip(">")?..];
        } else {
            // A tag ends at the first `>` outside a quoted attribute value.
            let mut quote = None;
            let end = rest.char_indices().find_map(|(at, c)| match (quote, c) {
                (None, '"' | '\'') => {
                    quote = Some(c);
                    None
                }
                (Some(q), c) if c == q => {
                    quote = None;
                    None
                }
                (None, '>') => Some(at),
                _ => None,
            })?;
            let tag = &rest[1..end];
            rest = &rest[end + 1..];
            let closing = tag.starts_with('/');
            let name = tag.trim_start_matches('/');
            let name = name.split(|c: char| c.is_whitespace() || c == '/').next();
            let name = name.unwrap_or("");
            let local = name.rsplit(':').next().unwrap_or(name);
            if closing {
                if at_number(&path) {
                    zero |= number.trim() == "0";
                    number.clear();
                } else if path.len() == 3 && path[2] == "Task" {
                    count += usize::from(std::mem::take(&mut zero));
                }
                path.pop();
            } else if !tag.ends_with('/') {
                path.push(local);
            }
        }
    }
    (count > 0).then_some(count)
}

/// How many tasks of the opened MS Project file state both dates of their Baseline 0, which is what
/// opening the file keeps as the session's baseline.
fn dated_baselines(extras: &Extras<'_>) -> usize {
    let Some(source) = extras.source.filter(|s| s.format == SourceFormat::Mspdi) else {
        return 0;
    };
    let Ok(read) = import::parse_mspdi(&String::from_utf8_lossy(source.bytes)) else {
        return 0;
    };
    read.tasks
        .iter()
        .filter(|t| t.baseline_start.is_some() && t.baseline_finish.is_some())
        .count()
}

/// The tasks whose start differs when `bytes` is read back and scheduled: their index in the file
/// and the start they get. Empty when the file cannot be read back.
fn drift(bytes: &[u8], expected: &[Option<Instant>]) -> Vec<(usize, Instant)> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let Ok(mut read) = import::parse_mspdi(text) else {
        return Vec::new();
    };
    let _ = read.schedule.set_data_date(read.data_date);
    // The data date and the actuals shape the first compute, as `load_bytes` does.
    for (at, task) in read.tasks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
        )]
        let id = TaskId(at as u32);
        let _ = read.schedule.set_actual_start(id, task.actual_start);
        let _ = read.schedule.set_actual_finish(id, task.actual_finish);
        let _ = read.schedule.set_remaining(id, task.remaining);
    }
    let Ok(solution) = read.schedule.compute() else {
        return Vec::new();
    };
    expected
        .iter()
        .enumerate()
        .filter_map(|(at, &start)| {
            let start = start?;
            let got = solution.dates.get(at)?.es;
            (got != start).then_some((at, got))
        })
        .collect()
}

/// The session's tasks as an [`Imported`] whose task `i` is the `i`th live task in outline order,
/// and the start each task has in the session (`None` for a task with sub-tasks).
///
/// `floor` sets every task's start-no-earlier-than to at least the project start, which an MS
/// Project file needs; an XER keeps each task's own constraint, and `export_xer` sets its start.
#[expect(
    clippy::cast_possible_truncation,
    reason = "new and index are a task slot and a dependency index; Schedule::add_task and add_dependency cast their counts to u32 with no check against u32::MAX, so past 2^32 they wrap, as those ids do"
)]
fn imported(
    session: &Session,
    extras: &Extras<'_>,
    floor: bool,
) -> (Imported, Vec<Option<Instant>>) {
    let live = session.schedule();
    let order = live_order(session);
    let level = |at: usize| session.meta()[order[at].0 as usize].outline_level;
    let starts: Vec<Option<Instant>> = (0..order.len())
        .map(|at| {
            let parent = at + 1 < order.len() && level(at + 1) > level(at);
            (!parent).then(|| session.solution().get(order[at]).es)
        })
        .collect();
    let project_start = starts.iter().flatten().min().copied();
    let at: HashMap<TaskId, TaskId> = order
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, TaskId(new as u32)))
        .collect();

    let mut schedule = empty_like(live);
    for &old in &order {
        let mut task = live.tasks()[old.0 as usize].clone();
        // The file floors every task at the project start.
        if floor {
            task.snet = Some(task.snet.unwrap_or(0).max(project_start.unwrap_or(0)));
        }
        let _ = schedule.add_task(task);
    }
    for (index, dep) in live.dependencies().iter().enumerate() {
        let id = DepId(index as u32);
        if !live.is_dependency_live(id) {
            continue;
        }
        let (Some(&pred), Some(&succ)) = (at.get(&dep.pred), at.get(&dep.succ)) else {
            continue;
        };
        let linked = Dependency { pred, succ, ..*dep };
        if let (Ok(new), Some(own)) = (
            schedule.add_dependency(linked),
            live.dependency_lag_calendar(id),
        ) {
            let _ = schedule.set_dependency_lag_calendar(new, Some(own));
        }
    }

    let shown = session.timeline().progress();
    let tasks = order
        .iter()
        .map(|&old| {
            let meta = &session.meta()[old.0 as usize];
            let task = &live.tasks()[old.0 as usize];
            let baseline = session.baseline().and_then(|b| b.task(old));
            ImportedTask {
                uid: old.0 + 1,
                code: meta.code.clone(),
                name: meta.name.clone(),
                outline_level: meta.outline_level,
                parent: meta.parent.and_then(|p| at.get(&p).copied()),
                actual_start: task.actual_start,
                actual_finish: task.actual_finish,
                remaining: task.remaining,
                in_progress: task.actual_start.is_some() && task.actual_finish.is_none(),
                baseline_start: baseline.map(|b| b.es),
                baseline_finish: baseline.map(|b| b.ef),
                progress: percent(task, shown.get(old.0 as usize).copied()),
                activity_codes: meta.activity_codes.clone(),
                wbs_code: meta.wbs_code.clone(),
                deadline: session.deadline(old),
                // An export states no file start, file finish or floor.
                ..ImportedTask::default()
            }
        })
        .collect();

    let imported = Imported {
        project_start: if floor { project_start } else { None },
        schedule,
        tasks,
        unsupported: extras.notes.to_vec(),
        data_date: session.data_date(),
        origin_day: session.origin().day(),
    };
    (imported, starts)
}

/// A schedule with no tasks that keeps what `live` holds apart from them: its calendars, lag calendar, data
/// date and options.
fn empty_like(live: &Schedule) -> Schedule {
    let mut schedule = Schedule::new();
    for calendar in live.calendars() {
        schedule.add_calendar(calendar.clone());
    }
    let _ = schedule.set_lag_calendar(live.lag_calendar());
    let _ = schedule.set_data_date(live.data_date());
    schedule.set_options(live.options());
    schedule
}

/// Percent complete from the engine's own facts: 100 with an actual finish, the share of the
/// duration no longer remaining once started. A task with no actuals keeps the percent the
/// timeline shows, which is the file's own figure for an opened file.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "remaining is clamped to 0 to duration and duration is positive, so done is 0 to 100; the product by 100 overflows only for a duration past i64::MAX / 100, which nothing here checks"
)]
fn percent(task: &Task, shown: Option<u8>) -> Option<u8> {
    if task.actual_finish.is_some() {
        return Some(100);
    }
    if task.actual_start.is_none() {
        return shown;
    }
    let done = match task.remaining {
        Some(remaining) if task.duration > 0 => {
            (task.duration - remaining.clamp(0, task.duration)) * 100 / task.duration
        }
        _ => 0,
    };
    Some(done as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{demo, fitted_timeline};
    use engine::UnstartedRemaining;

    #[test]
    fn an_export_says_the_schedule_options_and_the_finish_date_are_not_written() {
        let (mut schedule, solution, order, meta) = demo(3);
        let set = ScheduleOptions {
            unstarted_remaining: UnstartedRemaining::On,
            ..ScheduleOptions::default()
        };
        schedule.set_options(set);
        schedule.set_finish_by(Some(5_000));
        let timeline = fitted_timeline(order, &solution, 1000.0, 400.0, 1.0);
        let session = Session::new(schedule, solution, timeline, meta, || 0.0);
        let extras = Extras {
            name: "export",
            notes: &[],
            source: None,
        };
        let lines = [
            "the schedule options are not written: the file reopens on the defaults",
            "the must-finish-by date is not written: the file reopens with no date",
        ];
        for out in [
            export_xer(&session, &extras),
            export_mspdi(&session, &extras),
        ] {
            let left_out = out.unwrap().left_out;
            assert!(lines.iter().all(|line| left_out.iter().any(|l| l == line)));
        }
    }
}
