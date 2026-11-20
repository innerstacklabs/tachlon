//! An MS Project task's `ActualStart`, `ActualFinish` and `RemainingDuration`, as the engine takes
//! them (decision 0008) and as the XER reader hands them over: a started task keeps its actual
//! start and finishes its remaining duration after the data date; a finished one is fixed at its
//! actuals. `PercentComplete` is display only and moves no date.

use engine::{Instant, Minutes};

use super::relative_instant;
use crate::ImportError;
use crate::dom::Node;
use crate::fields::invalid;
use crate::notes::Notes;
use crate::range;
use crate::units::parse_duration_minutes;

/// What the file states of a task's progress.
#[derive(Default)]
pub(super) struct Actuals {
    pub(super) start: Option<Instant>,
    pub(super) finish: Option<Instant>,
    pub(super) remaining: Option<Minutes>,
    pub(super) resume: Option<Instant>,
}

impl Actuals {
    /// The actuals of a non-summary task; a summary takes none. The finish and the remaining
    /// duration are read only with a start, as for a P6 activity. An unreadable date or
    /// duration is treated as absent.
    pub(super) fn of(
        node: &Node,
        summary: bool,
        origin_day: i64,
        uid: u32,
        notes: &mut Notes,
    ) -> Self {
        if summary {
            return Self::default();
        }
        let id = uid.to_string();
        let stated = |element: &str| node.child_text(element).filter(|value| !value.is_empty());
        let instant = |element: &str, notes: &mut Notes| {
            stated(element)
                .and_then(|value| relative_instant(value, origin_day, element).ok())
                .and_then(|at| range::date(at, uid, &id, notes))
        };
        let start = instant("ActualStart", notes);
        let finish = start.and_then(|_| instant("ActualFinish", notes));
        let remaining = start
            .and_then(|_| stated("RemainingDuration"))
            .and_then(parse_duration_minutes)
            .map(|(minutes, _)| minutes)
            .filter(|minutes| *minutes >= 0)
            .and_then(|minutes| range::remaining(minutes, &id, notes));
        let resume = start
            .filter(|_| finish.is_none())
            .and_then(|_| instant("Resume", notes));
        Self {
            start,
            finish,
            remaining,
            resume,
        }
    }
}

/// One note for each actual the reader leaves out of a non-summary task, as [`Actuals::of`] reads
/// the task: an `ActualStart` or `ActualFinish` that is not a date, an `ActualFinish` with no
/// usable `ActualStart`, a `Resume` that is not a date, has no usable `ActualStart` or sits on a
/// finished task, a `Stop` date, and an `ActualDuration` on a task with no actual dates. A summary has none of these listed (the app rolls it up). A
/// `RemainingDuration` alone is not listed: MS Project writes one on every task.
pub(super) fn note_dropped(
    node: &Node,
    uid: u32,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    let stated = |element: &str| node.child_text(element).filter(|value| !value.is_empty());
    let readable =
        |element: &str, value: &str| relative_instant(value, origin_day, element).is_ok();
    let start = stated("ActualStart");
    let finish = stated("ActualFinish");
    let start_usable = start.is_some_and(|value| readable("ActualStart", value));
    if let Some(value) = start.filter(|_| !start_usable) {
        notes.line(format!(
            "Task {uid}: the actual start {value:?} is not a date, so its actual dates were not read; the task is scheduled as not started and its dates can differ from the file's."
        ));
    }
    match finish {
        Some(value) if !readable("ActualFinish", value) => notes.line(format!(
            "Task {uid}: the actual finish {value:?} is not a date and was not read; the task is not fixed at a finish and its dates can differ from the file's."
        )),
        Some(_) if !start_usable => notes.line(format!(
            "Task {uid}: the actual finish was not read because the task has no actual start; its dates can differ from the file's."
        )),
        _ => {}
    }
    let finished = finish.is_some_and(|value| readable("ActualFinish", value));
    if let Some(value) =
        stated("Resume").filter(|value| !readable("Resume", value) || !start_usable || finished)
    {
        notes.line(format!(
            "Task {uid}: the resume date {value:?} was not read; the task is not held to it and its dates can differ from the file's."
        ));
    }
    if stated("Stop").is_some() {
        notes.line(format!(
            "Task {uid}: the stop date was not read; no dates are affected."
        ));
    }
    if start.is_none()
        && finish.is_none()
        && let Some(value) = node.child_text("ActualDuration")
    {
        let (minutes, _) =
            parse_duration_minutes(value).ok_or_else(|| invalid("ActualDuration", value))?;
        if minutes != 0 {
            notes.line(format!(
                "Task {uid}: the actual duration was not read because the task has no actual dates; no dates are affected."
            ));
        }
    }
    Ok(())
}
