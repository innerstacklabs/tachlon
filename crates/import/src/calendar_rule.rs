//! The base-calendar rule both importers share: how a calendar's own week and exceptions
//! combine with the calendars it is based on, and the default week used when a file has none.
//!
//! The importers read their formats into [`Layer`]s; [`resolve`] walks the base chain and
//! returns the combined week and exceptions. Anything that goes wrong in the chain comes back
//! as a [`ChainWarning`] and the caller words the note for its format.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use engine::DayPeriod;

use crate::ImportError;
use crate::notes::Notes;

/// A date (days from the schedule origin) and the periods that replace the weekly pattern
/// on it. No periods is a holiday.
pub(crate) type Exception = (i64, Vec<DayPeriod>);

/// Working periods for each weekday, Monday first.
pub(crate) type Week = [Vec<DayPeriod>; 7];

/// What one calendar states for itself, before any base calendar is applied.
pub(crate) struct Layer {
    /// The calendar this one is based on, if it names one.
    pub base: Option<u32>,
    /// Each weekday this calendar states, Monday first. `None` is a day it leaves to its base,
    /// so a layer that states every day replaces the whole week.
    pub week: [Option<Vec<DayPeriod>>; 7],
    /// This calendar's own exceptions, at most one per date.
    pub exceptions: Vec<Exception>,
    /// The dates this calendar's work weeks set (MSPDI), at most one per date and none on a
    /// date in `exceptions`. A work week replaces a base calendar's working exception on its
    /// dates, but not a base calendar's holiday.
    pub work_week: Vec<Exception>,
}

/// Whether a calendar takes its base calendars' exceptions as well as their weeks.
#[derive(Clone, Copy)]
pub(crate) enum Holidays {
    /// The base calendars' exceptions apply, and a derived calendar's exception replaces its
    /// base calendar's on the same date (MSPDI).
    Inherit,
    /// Only the starting calendar's own exceptions apply (XER).
    Own,
}

/// A problem found while walking the base chain. The walk stops at the problem and uses
/// the calendars it reached.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChainWarning {
    /// Calendar `uid` names a base calendar `base` that the file does not define.
    BaseMissing { uid: u32, base: u32 },
    /// The base chain of calendar `uid` loops back on itself.
    Cycle { uid: u32 },
}

/// A calendar's week and exceptions after its base chain is applied.
pub(crate) struct Resolved {
    pub week: Week,
    /// The nearest calendar, starting with `start` itself, that states at least one weekday.
    /// `None` when no calendar in the chain does, and `week` is then empty.
    pub week_from: Option<u32>,
    /// Sorted by date.
    pub exceptions: Vec<Exception>,
    pub warnings: Vec<ChainWarning>,
}

/// Combine calendar `start` with the calendars it is based on. `layer_of` finds a calendar by
/// its uid. A base calendar's weekdays apply first and each derived calendar's stated weekdays
/// replace them; `holidays` says what happens to exceptions.
///
/// A start calendar that `layer_of` cannot find is an error. A missing base or a cycle further
/// along the chain is a warning, never an error, and never loops.
pub(crate) fn resolve<'a>(
    start: u32,
    layer_of: impl Fn(u32) -> Option<&'a Layer>,
    holidays: Holidays,
) -> Result<Resolved, ImportError> {
    let missing = || ImportError::Missing {
        element: "Calendar".to_owned(),
    };
    let mut chain: Vec<(u32, &Layer)> = Vec::new();
    let mut warnings = Vec::new();
    let mut current = start;
    let mut layer = layer_of(current).ok_or_else(missing)?;
    loop {
        chain.push((current, layer));
        let Some(base) = layer.base else {
            break;
        };
        let Some(base_layer) = layer_of(base) else {
            warnings.push(ChainWarning::BaseMissing { uid: current, base });
            break;
        };
        if chain.iter().any(|(uid, _)| *uid == base) {
            warnings.push(ChainWarning::Cycle { uid: start });
            break;
        }
        current = base;
        layer = base_layer;
    }

    let mut week: Week = std::array::from_fn(|_| Vec::new());
    let mut exceptions: BTreeMap<i64, Vec<DayPeriod>> = BTreeMap::new();
    // The dates that an exception of a calendar in the chain so far makes a holiday.
    let mut holidays_so_far: BTreeSet<i64> = BTreeSet::new();
    for &(uid, layer) in chain.iter().rev() {
        for (day, stated) in week.iter_mut().zip(&layer.week) {
            if let Some(periods) = stated {
                day.clone_from(periods);
            }
        }
        if matches!(holidays, Holidays::Inherit) || uid == start {
            exceptions.extend(
                layer
                    .work_week
                    .iter()
                    .filter(|(day, _)| !holidays_so_far.contains(day))
                    .cloned(),
            );
            for (day, periods) in &layer.exceptions {
                if periods.is_empty() {
                    holidays_so_far.insert(*day);
                } else {
                    holidays_so_far.remove(day);
                }
            }
            exceptions.extend(layer.exceptions.iter().cloned());
        }
    }
    let week_from = chain
        .iter()
        .find(|(_, layer)| layer.week.iter().any(Option::is_some))
        .map(|(uid, _)| *uid);
    Ok(Resolved {
        week,
        week_from,
        exceptions: exceptions.into_iter().collect(),
        warnings,
    })
}

/// One exception per date for the engine, which refuses two on one day. The first stays.
/// A repeat of the same periods (two holidays on one date) loses nothing and is silent; a
/// repeat that differs is dropped and counted in one note.
pub(crate) fn dedupe(entries: Vec<Exception>, uid: u32, notes: &mut Notes) -> Vec<Exception> {
    let (kept, dropped) = dedupe_counting(entries);
    if dropped > 0 {
        notes.line(format!(
            "Calendar {uid}: exceptions that repeat a date with different hours were not read ({dropped}); the first on each date is used, so some days can differ from the file's."
        ));
    }
    kept
}

/// [`dedupe`] without the note: the entries that stay and how many differing repeats it
/// dropped, for a caller that words and places the note itself.
pub(crate) fn dedupe_counting(entries: Vec<Exception>) -> (Vec<Exception>, usize) {
    let mut kept: Vec<Exception> = Vec::with_capacity(entries.len());
    let mut index: HashMap<i64, usize> = HashMap::new();
    let mut dropped = 0usize;
    for entry in entries {
        match index.get(&entry.0).and_then(|&at| kept.get(at)) {
            Some(first) if first.1 == entry.1 => {}
            Some(_) => dropped += 1,
            None => {
                index.insert(entry.0, kept.len());
                kept.push(entry);
            }
        }
    }
    (kept, dropped)
}

/// The week used when a file gives no calendar: Monday to Friday, 08:00 to 17:00.
pub(crate) fn fallback_week() -> Week {
    let mut week: Week = std::array::from_fn(|_| Vec::new());
    for day in week.iter_mut().take(5) {
        day.push((480, 1_020));
    }
    week
}

#[cfg(test)]
mod tests;
