//! An XER activity's constraints: the ones that map onto the engine, and the notes for the ones
//! that do not.

use engine::{Limit, Pin};
use xer::{ConstraintType, TaskRow};

use super::ascii;
use crate::ImportError;
use crate::notes::{Notes, Section};
use crate::range;

/// What an activity's two constraints give the engine.
#[derive(Default)]
pub(super) struct Constraints {
    pub(super) snet: Option<i64>,
    pub(super) limit: Limit,
}

/// Where an activity's constraints are read from: its label for the notes, the schedule origin
/// and its id for an error.
#[derive(Clone, Copy)]
struct Source<'a> {
    pub(super) label: &'a str,
    pub(super) origin_day: i64,
    pub(super) task_id: u32,
}

/// Read the primary and the secondary constraint. Both bind: when they set the same bound, the
/// stricter one holds. A constraint the reader cannot apply stays an import note.
pub(super) fn constraint(
    activity: &TaskRow,
    label: &str,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<Constraints, ImportError> {
    let source = Source {
        label,
        origin_day,
        task_id: activity.task_id,
    };
    let mut out = Constraints::default();
    let kinds = [
        (&activity.cstr_type, activity.cstr_date, "the"),
        (&activity.cstr_type2, activity.cstr_date2, "the secondary"),
    ];
    for (kind, date, which) in kinds {
        if let Some(kind) = kind {
            apply(&mut out, kind, date, which, source, notes)?;
        }
    }
    Ok(out)
}

fn apply(
    out: &mut Constraints,
    kind: &ConstraintType,
    date: Option<i64>,
    which: &str,
    source: Source<'_>,
    notes: &mut Notes,
) -> Result<(), ImportError> {
    let not_read = |notes: &mut Notes, when: &str| {
        notes.add(
            Section::Activity,
            &format!("Activities with {which} {} constraint{when}: the constraint was not read and they are scheduled without it, so their dates can differ from the file's", constraint_words(kind)),
            Some(source.label),
        );
    };
    match kind {
        ConstraintType::AsLateAsPossible => out.limit.alap = true,
        ConstraintType::Other(_) => not_read(notes, ""),
        _ => {
            let Some(minutes) = date else {
                not_read(notes, " but no date");
                return Ok(());
            };
            let at = relative_instant(minutes, source.origin_day, source.task_id)?;
            let Some(at) = range::date(at, source.label, notes) else {
                return Ok(());
            };
            let at = at.max(0);
            let mut not_read = |when: &str| not_read(notes, when);
            match kind {
                ConstraintType::StartOnOrAfter => later(&mut out.snet, at),
                ConstraintType::FinishOnOrAfter => later(&mut out.limit.fnet, at),
                ConstraintType::StartOnOrBefore => earlier(&mut out.limit.snlt, at),
                ConstraintType::FinishOnOrBefore => earlier(&mut out.limit.fnlt, at),
                ConstraintType::StartOn | ConstraintType::MandatoryStart => {
                    pin(&mut out.limit, Pin::Start(at), &mut not_read);
                }
                _ => pin(&mut out.limit, Pin::Finish(at), &mut not_read),
            }
        }
    }
    Ok(())
}

/// A lower bound the activity already has stays when it is the later one.
fn later(slot: &mut Option<i64>, at: i64) {
    *slot = Some(slot.map_or(at, |held| held.max(at)));
}

/// An upper bound the activity already has stays when it is the earlier one.
fn earlier(slot: &mut Option<i64>, at: i64) {
    *slot = Some(slot.map_or(at, |held| held.min(at)));
}

/// One date holds a task, so a second pin that differs from the first is left out.
fn pin(limit: &mut Limit, new: Pin, not_read: &mut impl FnMut(&str)) {
    match limit.pin {
        Some(held) if held != new => not_read(""),
        _ => limit.pin = Some(new),
    }
}

fn relative_instant(minutes: i64, origin_day: i64, task_id: u32) -> Result<i64, ImportError> {
    origin_day
        .checked_mul(1_440)
        .and_then(|origin| minutes.checked_sub(origin))
        .ok_or_else(|| ImportError::Invalid {
            element: format!("activity {task_id} constraint date"),
            value: minutes.to_string(),
        })
}

fn constraint_words(kind: &ConstraintType) -> String {
    match kind {
        ConstraintType::StartOn => "must start on".to_owned(),
        ConstraintType::StartOnOrAfter => "start on or after".to_owned(),
        ConstraintType::StartOnOrBefore => "start on or before".to_owned(),
        ConstraintType::FinishOn => "must finish on".to_owned(),
        ConstraintType::FinishOnOrAfter => "finish on or after".to_owned(),
        ConstraintType::FinishOnOrBefore => "finish on or before".to_owned(),
        ConstraintType::AsLateAsPossible => "as late as possible".to_owned(),
        ConstraintType::MandatoryStart => "mandatory start".to_owned(),
        ConstraintType::MandatoryFinish => "mandatory finish".to_owned(),
        ConstraintType::Other(code) => format!("unrecognised ({})", ascii(code)),
    }
}
