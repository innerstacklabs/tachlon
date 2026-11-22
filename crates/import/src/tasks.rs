use engine::{CalendarId, Limit, Pin, Schedule, Task, TaskId};

use crate::calendars::Calendars;
use crate::deadline;
use crate::dom::Node;
use crate::fields::{
    elapsed_format, flag, format_code, invalid, optional_i64, optional_u32, required_u32,
};
use crate::floor::Floor;
use crate::notes::{Notes, Section};
use crate::range;
use crate::units::{parse_datetime, parse_duration_minutes};
use crate::{ImportError, ImportedTask};
use actuals::Actuals;
use baseline::baseline_dates;
pub(crate) use list::Tasks;

mod actuals;
mod baseline;
mod links;
mod list;
mod memo;

/// The task's duration in minutes, and whether those minutes are elapsed time: an elapsed
/// `DurationFormat` on a task that keeps a duration. The caller counts an elapsed duration in
/// the all-minutes calendar, so it spans weekends and holidays.
fn duration(node: &Node, uid: u32, notes: &mut Notes) -> Result<(i64, bool), ImportError> {
    let mut minutes = match node.child_text("Duration") {
        Some(value) => {
            let (minutes, truncated) =
                parse_duration_minutes(value).ok_or_else(|| invalid("Duration", value))?;
            if truncated {
                notes.line(format!(
                    "Task {uid}: the seconds in its duration were not read; the duration is rounded down to whole minutes, so its finish can differ by under a minute."
                ));
            }
            minutes
        }
        None => 0,
    };
    let elapsed = format_code(node, "DurationFormat")?
        .is_some_and(|(_, normalized)| elapsed_format(normalized));
    if flag(node, "Milestone")? == Some(true) {
        if minutes != 0 {
            notes.line(format!(
                "Task {uid}: the milestone's duration of {minutes} minutes was not read; a milestone takes no time, so dates after it can differ from the file's."
            ));
        }
        minutes = 0;
    }
    let minutes = range::duration(minutes, &uid.to_string(), notes);
    Ok((minutes, elapsed && minutes != 0))
}

fn task_calendar(
    node: &Node,
    uid: u32,
    calendars: &Calendars,
    notes: &mut Notes,
) -> Result<CalendarId, ImportError> {
    let Some(value) = node.child_text("CalendarUID") else {
        return Ok(calendars.default);
    };
    if value == "-1" {
        return Ok(calendars.default);
    }
    let calendar_uid: u32 = value.parse().map_err(|_| invalid("CalendarUID", value))?;
    if let Some(calendar) = calendars.by_uid.get(&calendar_uid).copied() {
        Ok(calendar)
    } else {
        notes.line(format!(
            "Task {uid}: calendar {calendar_uid} is not in the file, so the project calendar is used and its dates can differ from the file's."
        ));
        Ok(calendars.default)
    }
}

pub(crate) fn relative_instant(
    value: &str,
    origin_day: i64,
    element: &str,
) -> Result<i64, ImportError> {
    let (day, minute) = parse_datetime(value).ok_or_else(|| invalid(element, value))?;
    day.checked_sub(origin_day)
        .and_then(|days| days.checked_mul(1440))
        .and_then(|minutes| minutes.checked_add(i64::from(minute)))
        .ok_or_else(|| invalid(element, value))
}

fn constraint_name(code: u32) -> &'static str {
    match code {
        1 => "as late as possible",
        2 => "must start on",
        3 => "must finish on",
        5 => "start no later than",
        6 => "finish no earlier than",
        7 => "finish no later than",
        _ => "unknown",
    }
}

/// What a task's constraint gives the engine: a start-no-earlier-than date, which the project
/// start floors, and the other limits.
#[derive(Default)]
struct Constraint {
    snet: Option<i64>,
    limit: Limit,
}

/// Read the task's `ConstraintType`: as late as possible needs no date, the others take the
/// `ConstraintDate`. A type with no date, and a type the reader does not know, stay import notes.
fn constraint(
    node: &Node,
    uid: u32,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<Constraint, ImportError> {
    let code = optional_u32(node, "ConstraintType")?.unwrap_or(0);
    let mut out = Constraint::default();
    let mut not_read = |why: &str| {
        notes.line(format!(
            "Task {uid}: the {} constraint (type {code}){why} was not read; the task is scheduled without it, so its dates can differ from the file's.",
            constraint_name(code)
        ));
    };
    match code {
        0 => {}
        1 => out.limit.alap = true,
        2..=7 => {
            let date = node
                .child_text("ConstraintDate")
                .filter(|value| !value.is_empty());
            if code != 4 && date.is_none() {
                not_read(" has no date and");
                return Ok(out);
            }
            let at = relative_instant(date.unwrap_or_default(), origin_day, "ConstraintDate")?;
            let Some(at) = range::date(at, uid, &uid.to_string(), notes) else {
                return Ok(out);
            };
            let at = at.max(0);
            match code {
                2 => out.limit.pin = Some(Pin::Start(at)),
                3 => out.limit.pin = Some(Pin::Finish(at)),
                4 => out.snet = Some(at),
                5 => out.limit.snlt = Some(at),
                6 => out.limit.fnet = Some(at),
                _ => out.limit.fnlt = Some(at),
            }
        }
        _ => not_read(""),
    }
    Ok(out)
}

const INACTIVE: &str = "Tasks that are inactive in the file: they were imported as active, so they take part in the schedule and their dates can differ from the file's";
const MANUAL: &str = "Tasks scheduled manually in the file: they were imported as automatically scheduled, so their dates can differ from the file's";
const LEVELING_DELAY: &str = "Tasks with a leveling delay: the delay was not read, so their dates can differ from the file's";

/// The notes a task earns. The counted ones (`Deadline`, `Active`, `Manual`, `LevelingDelay`)
/// list before every sentence. The note for a resource assignment goes before the sentences this
/// task earns: the assignments are read after the tasks, so `assigned_at` is told where that note
/// belongs (a position among the sentences in `notes`).
fn task_notes(
    node: &Node,
    uid: u32,
    assigned_at: &mut usize,
    summary: bool,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    if node
        .child_text("Deadline")
        .is_some_and(|value| !value.is_empty())
        && deadline::read(node, origin_day).is_none()
    {
        notes.add(
            Section::Activity,
            deadline::NOT_READ,
            Some(&uid.to_string()),
        );
    }
    if flag(node, "Active")? == Some(false) {
        notes.add(Section::Activity, INACTIVE, Some(&uid.to_string()));
    }
    *assigned_at = notes.len();
    if flag(node, "Manual")? == Some(true) {
        notes.add(Section::Activity, MANUAL, Some(&uid.to_string()));
    }
    if !summary {
        actuals::note_dropped(node, uid, origin_day, notes)?;
    }
    if optional_i64(node, "LevelingDelay")?.is_some_and(|delay| delay != 0) {
        notes.add(Section::Activity, LEVELING_DELAY, Some(&uid.to_string()));
    }
    memo::note_dropped(node, notes);
    Ok(())
}

/// The task's `PhysicalPercentComplete` in hundredths of a percent. MS Project writes a 0 on tasks
/// that have none, so a 0 reads as none.
fn physical_percent(node: &Node, notes: &mut Notes) -> Result<Option<u16>, ImportError> {
    let percent = percent_element(node, "PhysicalPercentComplete", notes)?;
    Ok(percent
        .filter(|&percent| percent > 0)
        .map(|percent| u16::from(percent) * 100))
}

/// The task's `PercentComplete`, clamped to 0 to 100 with one note; display only.
fn percent_complete(node: &Node, notes: &mut Notes) -> Result<Option<u8>, ImportError> {
    percent_element(node, "PercentComplete", notes)
}

fn percent_element(node: &Node, name: &str, notes: &mut Notes) -> Result<Option<u8>, ImportError> {
    let Some(text) = node.child_text(name).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let value: i64 = text.parse().map_err(|_| invalid(name, text))?;
    if !(0..=100).contains(&value) {
        notes.line(
            "A percent complete outside 0 to 100 was changed to the nearest limit; no dates are affected."
                .to_owned(),
        );
    }
    #[expect(
        clippy::cast_sign_loss,
        reason = "the value is clamped to 0..=100, so it is not negative and fits u8"
    )]
    let percent = value.clamp(0, 100) as u8;
    Ok(Some(percent))
}

/// The file's `Start` and `Finish` as instants from the schedule origin, and whether the task is
/// in progress (`ActualStart` stated, no `ActualFinish`), as the XER reader does for `TK_Active`.
/// A summary has none of these; an unreadable `Start` or `Finish` is `None`.
fn file_dates(node: &Node, summary: bool, origin_day: i64) -> (Option<i64>, Option<i64>, bool) {
    if summary {
        return (None, None, false);
    }
    let stated = |element: &str| node.child_text(element).filter(|value| !value.is_empty());
    let instant = |element: &str| {
        stated(element).and_then(|value| relative_instant(value, origin_day, element).ok())
    };
    let in_progress = stated("ActualStart").is_some() && stated("ActualFinish").is_none();
    (instant("Start"), instant("Finish"), in_progress)
}

fn parent_for(level: u8, open: &mut Vec<(TaskId, u8)>) -> Option<TaskId> {
    while open.last().is_some_and(|&(_, l)| l >= level) {
        open.pop();
    }
    open.last().map(|&(id, _)| id)
}

/// A task read, with the position in the notes of its assignment note and the floor it needs.
struct Read {
    imported: ImportedTask,
    floor: Floor,
    assigned_at: usize,
}

fn read_task(
    node: &Node,
    schedule: &mut Schedule,
    calendars: &Calendars,
    origin_day: i64,
    open: &mut Vec<(TaskId, u8)>,
    notes: &mut Notes,
) -> Result<Read, ImportError> {
    let uid = required_u32(node, "UID")?;
    let outline_level = optional_u32(node, "OutlineLevel")?
        .map(|value| u8::try_from(value).map_err(|_| invalid("OutlineLevel", &value.to_string())))
        .transpose()?
        .unwrap_or(1);
    let summary = flag(node, "Summary")? == Some(true);
    let (mut task_duration, elapsed) = duration(node, uid, notes)?;
    let mut calendar = task_calendar(node, uid, calendars, notes)?;
    let Constraint {
        mut snet,
        mut limit,
    } = constraint(node, uid, origin_day, notes)?;
    if elapsed && !summary {
        // Elapsed time ignores the task's own calendar: it runs through nights and weekends.
        calendar = calendars.elapsed(schedule)?;
    }
    if summary {
        task_duration = 0;
        calendar = calendars.default;
        snet = None;
        limit = Limit::default();
    }
    let mut assigned_at = 0;
    task_notes(node, uid, &mut assigned_at, summary, origin_day, notes)?;
    let (file_start, file_finish, in_progress) = file_dates(node, summary, origin_day);
    let actuals = Actuals::of(node, summary, origin_day, uid, notes);
    limit.resume = actuals.resume;
    let (baseline_start, baseline_finish) = baseline_dates(node, origin_day);
    let id = schedule.add_task(Task {
        duration: task_duration,
        calendar,
        snet,
        limit,
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    })?;
    let imported = ImportedTask {
        uid,
        code: None,
        name: node
            .child("Name")
            .map_or_default(|name| name.text.to_string()),
        outline_level,
        parent: parent_for(outline_level, open),
        file_start,
        file_finish,
        in_progress,
        actual_start: actuals.start,
        actual_finish: actuals.finish,
        remaining: actuals.remaining,
        baseline_start,
        baseline_finish,
        progress: percent_complete(node, notes)?,
        physical: physical_percent(node, notes)?,
        activity_codes: Vec::new(),
        wbs_code: node
            .child_text("WBS")
            .filter(|code| !code.is_empty())
            .map(str::to_owned),
        floor: None,
        deadline: deadline::read(node, origin_day),
        resource_names: Vec::new(),
    };
    Ok(Read {
        imported,
        floor: Floor {
            id,
            project_start: None,
            own_snet: snet,
        },
        assigned_at,
    })
}

#[cfg(test)]
mod tests {
    use crate::parse_mspdi;

    #[test]
    fn parses_three_tasks_and_computes_one_finish_start_link() {
        let xml = "<Project><Tasks>\
            <Task><UID>1</UID><Name>First</Name><Duration>PT1H</Duration></Task>\
            <Task><UID>2</UID><Name>Second</Name><Duration>PT2H</Duration>\
              <PredecessorLink><PredecessorUID>1</PredecessorUID></PredecessorLink>\
            </Task>\
            <Task><UID>3</UID><Name>Third</Name><Duration>PT30M</Duration></Task>\
            </Tasks></Project>";
        let mut imported = parse_mspdi(xml).expect("minimal project should parse");

        assert_eq!(imported.tasks.len(), 3);
        assert_eq!(imported.schedule.dependencies().len(), 1);
        imported
            .schedule
            .compute()
            .expect("minimal project should compute");
    }
}
