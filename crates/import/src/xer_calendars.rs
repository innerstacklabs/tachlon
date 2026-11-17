use std::collections::{HashMap, HashSet};

use engine::{Calendar, CalendarError, CalendarId, DayPeriod, Schedule};
use xer::CalendarType;

use crate::ImportError;
use crate::calendar_rule::{self, ChainWarning, Holidays, Layer};
use crate::notes::{Notes, Section};
use crate::periods::keep_valid;

/// Lazily converts XER calendars into engine calendars.
pub(crate) struct XerCalendars<'a> {
    data: &'a xer::Data,
    origin_day: i64,
    cache: HashMap<u32, Option<CalendarId>>,
    /// The week and base of each calendar met on a base chain so far, without exceptions:
    /// only the calendar being built contributes its own.
    layers: HashMap<u32, Layer>,
    /// How many invalid or overlapping periods each calendar in `layers` lost from its week.
    /// A calendar that takes that week reports them under its own id.
    dropped: HashMap<u32, usize>,
}

impl<'a> XerCalendars<'a> {
    pub(crate) fn new(data: &'a xer::Data, origin_day: i64) -> Self {
        Self {
            data,
            origin_day,
            cache: HashMap::new(),
            layers: HashMap::new(),
            dropped: HashMap::new(),
        }
    }

    pub(crate) fn project_default(
        &mut self,
        project: &xer::ProjectRow,
        schedule: &mut Schedule,
        notes: &mut Notes,
    ) -> Result<CalendarId, ImportError> {
        if let Some(clndr_id) = project.clndr_id
            && let Some(calendar) = self.build(clndr_id, schedule, notes)?
        {
            return Ok(calendar);
        }

        let kind = match project.clndr_id {
            Some(id) => format!(
                "Project {}: its calendar {} is missing or has no working time, so Monday to Friday 8:00 to 17:00 is used; working days can differ from the file's",
                project.proj_id, id
            ),
            None => format!(
                "Project {}: it names no calendar, so Monday to Friday 8:00 to 17:00 is used; working days can differ from the file's",
                project.proj_id
            ),
        };
        notes.add(Section::Project, &kind, None);
        Self::add_fallback(schedule, project.clndr_id.unwrap_or(0))
    }

    pub(crate) fn for_activity(
        &mut self,
        clndr_id: Option<u32>,
        project_default: CalendarId,
        activity: &str,
        schedule: &mut Schedule,
        notes: &mut Notes,
    ) -> Result<CalendarId, ImportError> {
        let Some(clndr_id) = clndr_id else {
            notes.add(
                Section::Calendar,
                "Activities with no calendar are scheduled on the project calendar, so their dates can differ from the file's",
                Some(activity),
            );
            return Ok(project_default);
        };
        if let Some(calendar) = self.build(clndr_id, schedule, notes)? {
            Ok(calendar)
        } else {
            let kind = format!(
                "Activities whose calendar {clndr_id} could not be used are scheduled on the project calendar, so their dates can differ from the file's"
            );
            notes.add(Section::Calendar, &kind, Some(activity));
            Ok(project_default)
        }
    }

    /// Note the calendar problems the reader found, for the calendars `projects` use only.
    pub(crate) fn report_warnings(&self, projects: &[&xer::ProjectRow], notes: &mut Notes) {
        used::report_warnings(self.data, &used::calendars(self.data, projects), notes);
    }

    #[expect(
        clippy::too_many_lines,
        reason = "follows one calendar chain and reports as it goes; splitting it would only move the same locals into arguments"
    )]
    fn build(
        &mut self,
        clndr_id: u32,
        schedule: &mut Schedule,
        notes: &mut Notes,
    ) -> Result<Option<CalendarId>, ImportError> {
        if let Some(cached) = self.cache.get(&clndr_id) {
            return Ok(*cached);
        }
        let Some(row) = self.data.calendar(clndr_id) else {
            self.cache.insert(clndr_id, None);
            return Ok(None);
        };
        // CA_Rsrc is unverified on a real P6 export (ruling 3, gs#116 corpus).
        if row.clndr_type == Some(CalendarType::Resource) {
            notes.add(
                Section::Calendar,
                &format!("Calendar {clndr_id}: it is a resource calendar and was not read; activities that use it get the project calendar, so their dates can differ from the file's"),
                None,
            );
            self.cache.insert(clndr_id, None);
            return Ok(None);
        }

        if let Some(CalendarType::Other(code)) = &row.clndr_type {
            notes.add(
                Section::Calendar,
                &format!("A calendar has the type {code}, which this import does not know; it was read as an ordinary calendar, no dates are affected"),
                Some(&clndr_id.to_string()),
            );
        }

        // P6's nominal day_hr_cnt does not define the actual weekly pattern, so it is not read.
        self.add_layers(clndr_id);

        // Exception notes wait in `pending` so that the notes about the week come first.
        let mut pending = Notes::new();
        let exceptions = exceptions::read(
            row.clndr_data.as_ref(),
            clndr_id,
            self.origin_day,
            &mut pending,
        )?;
        let own = Layer {
            base: row.base_clndr_id,
            week: self
                .layers
                .get(&clndr_id)
                .map_or_default(|layer| layer.week.clone()),
            exceptions,
            work_week: Vec::new(),
        };
        let layers = &self.layers;
        let resolved = calendar_rule::resolve(
            clndr_id,
            |uid| {
                if uid == clndr_id {
                    Some(&own)
                } else {
                    layers.get(&uid)
                }
            },
            Holidays::Own,
        )?;

        let week = if let Some(source) = resolved.week_from {
            let dropped = self.dropped.get(&source).copied().unwrap_or(0);
            notes.add_many(
                Section::Calendar,
                &format!("Calendar {clndr_id}: a working period is invalid or overlaps another and was not read; working hours can differ from the file's"),
                dropped,
            );
            if source != clndr_id {
                notes.add(
                    Section::Calendar,
                    "A calendar states no working week of its own, so its base calendar's week is used; working days can differ from the file's",
                    Some(&clndr_id.to_string()),
                );
            }
            resolved.week
        } else {
            notes.add(
                Section::Calendar,
                &format!(
                    "Calendar {clndr_id}: it states no working week, so Monday to Friday 8:00 to 17:00 is used; working days can differ from the file's"
                ),
                None,
            );
            calendar_rule::fallback_week()
        };
        // The chain is followed only until a calendar states a week; a problem further
        // along it is not reported.
        if resolved.week_from.is_none() {
            for warning in &resolved.warnings {
                report_chain_warning(warning, notes);
            }
        }
        // The week does not decide this: a calendar that states its own week loses a base's
        // holidays too. Whether P6 already copies them into the child's data is not verified.
        if self.base_chain_has_holidays(row.base_clndr_id, clndr_id) {
            notes.add(
                Section::Calendar,
                &format!("Calendar {clndr_id}: holidays listed on its base calendar are not applied to it; working days can differ from the file's"),
                None,
            );
        }

        notes.absorb(pending);

        let built = match Calendar::with_exceptions(week, &resolved.exceptions) {
            Ok(calendar) => Some(schedule.add_calendar(calendar)),
            Err(CalendarError::NoWorkingTime) => {
                notes.add(
                    Section::Calendar,
                    &format!("Calendar {clndr_id}: it has no working time and was not imported; activities that use it get the project calendar, so their dates can differ from the file's"),
                    None,
                );
                None
            }
            Err(source) => {
                return Err(ImportError::Calendar {
                    uid: clndr_id,
                    source,
                });
            }
        };
        self.cache.insert(clndr_id, built);
        Ok(built)
    }

    /// Whether any calendar above `start` on its base chain lists an exception. The walk ends at
    /// a missing calendar or one already met, so a loop cannot hold it.
    fn base_chain_has_holidays(&self, start: Option<u32>, clndr_id: u32) -> bool {
        let mut seen = HashSet::from([clndr_id]);
        let mut next = start;
        while let Some(id) = next {
            if !seen.insert(id) {
                return false;
            }
            let Some(row) = self.data.calendar(id) else {
                return false;
            };
            if row
                .clndr_data
                .as_ref()
                .is_some_and(|data| !data.exceptions.is_empty())
            {
                return true;
            }
            next = row.base_clndr_id;
        }
        false
    }

    /// Read the week and base of calendar `clndr_id` and of every calendar on its base chain
    /// that is not read yet. A chain that loops or names a missing calendar just ends.
    fn add_layers(&mut self, clndr_id: u32) {
        let mut next = Some(clndr_id);
        while let Some(id) = next {
            if self.layers.contains_key(&id) {
                break;
            }
            let Some(row) = self.data.calendar(id) else {
                break;
            };
            let (week, dropped) = match row.clndr_data.as_ref().and_then(|data| data.week.as_ref())
            {
                Some(source) => convert_week(source),
                None => (std::array::from_fn(|_| None), 0),
            };
            self.dropped.insert(id, dropped);
            next = row.base_clndr_id;
            self.layers.insert(
                id,
                Layer {
                    base: row.base_clndr_id,
                    week,
                    exceptions: Vec::new(),
                    work_week: Vec::new(),
                },
            );
        }
    }

    fn add_fallback(schedule: &mut Schedule, uid: u32) -> Result<CalendarId, ImportError> {
        Calendar::new(calendar_rule::fallback_week(), &[])
            .map(|calendar| schedule.add_calendar(calendar))
            .map_err(|source| ImportError::Calendar { uid, source })
    }
}

/// A week in engine order and how many periods it lost to `keep_valid`.
fn convert_week(source: &xer::Week) -> ([Option<Vec<DayPeriod>>; 7], usize) {
    // `f|00:00` and `24:00` as end of day are unverified on a real P6 export
    // (ruling 3, gs#116 corpus).
    let mut week = std::array::from_fn(|_| None);
    let mut dropped = 0usize;
    for (p6_index, source_periods) in source.iter().enumerate() {
        let engine_index = p6_index.saturating_add(6) % 7;
        let Some(day) = week.get_mut(engine_index) else {
            continue;
        };
        let (kept, lost) = keep_periods(source_periods);
        *day = Some(kept);
        dropped = dropped.saturating_add(lost);
    }
    (week, dropped)
}

/// One day's periods, sorted, and how many were dropped as invalid or overlapping.
fn keep_periods(source_periods: &[xer::Period]) -> (Vec<DayPeriod>, usize) {
    keep_valid(
        source_periods
            .iter()
            .map(|period| (period.start, period.finish))
            .collect(),
    )
}

/// One day's periods, sorted. An invalid or overlapping period is dropped and
/// counted, in a weekly day and in an exception day alike.
fn convert_periods(
    source_periods: &[xer::Period],
    clndr_id: u32,
    notes: &mut Notes,
) -> Vec<DayPeriod> {
    let (kept, dropped) = keep_periods(source_periods);
    for _ in 0..dropped {
        notes.add(
            Section::Calendar,
            &format!("Calendar {clndr_id}: a working period is invalid or overlaps another and was not read; working hours can differ from the file's"),
            None,
        );
    }
    kept
}

fn report_chain_warning(warning: &ChainWarning, notes: &mut Notes) {
    match warning {
        ChainWarning::BaseMissing { uid, base } => notes.add(
            Section::Calendar,
            &format!("Calendar {uid}: its base calendar {base} is not in the file, so the base's working time is not used; working days can differ from the file's"),
            None,
        ),
        ChainWarning::Cycle { uid } => notes.add(
            Section::Calendar,
            &format!("Calendar {uid}: its base calendars form a loop and the loop was not followed; working days can differ from the file's"),
            None,
        ),
    }
}

mod exceptions;
#[cfg(test)]
mod tests;
mod used;
