use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use engine::{Calendar, CalendarError, CalendarId, DayPeriod, Schedule};

use crate::ImportError;
use crate::calendar_rule::{self, ChainWarning, Holidays, Layer, dedupe};
use crate::dom::Node;
use crate::fields::{flag, invalid, required_text, required_u32};
use crate::notes::Notes;
use crate::periods::keep_valid;
use crate::units::parse_time_of_day;
use exceptions::{exception_days, work_week_days};

mod exceptions;

/// Calendars imported from an MSPDI project.
pub(crate) struct Calendars {
    /// The project default calendar.
    pub default: CalendarId,
    /// MSPDI calendar UID -> engine calendar id.
    pub by_uid: HashMap<u32, CalendarId>,
    /// The all-minutes calendar, added on the first call to `elapsed`.
    elapsed: OnceCell<CalendarId>,
}

impl Calendars {
    fn new(default: CalendarId, by_uid: HashMap<u32, CalendarId>) -> Calendars {
        Calendars {
            default,
            by_uid,
            elapsed: OnceCell::new(),
        }
    }

    /// The 24-hour, seven-day calendar a task with an elapsed duration counts in. It is added
    /// to `schedule` on first use, so a file with no elapsed duration gets no extra calendar.
    pub(crate) fn elapsed(&self, schedule: &mut Schedule) -> Result<CalendarId, ImportError> {
        if let Some(id) = self.elapsed.get() {
            return Ok(*id);
        }
        let week: [Vec<DayPeriod>; 7] = std::array::from_fn(|_| vec![(0, 1440)]);
        let calendar =
            Calendar::new(week, &[]).map_err(|source| ImportError::Calendar { uid: 0, source })?;
        let id = *self.elapsed.get_or_init(|| schedule.add_calendar(calendar));
        Ok(id)
    }
}

struct CalendarDefinition {
    uid: u32,
    is_base: bool,
    layer: Layer,
}

fn base_uid(node: &Node) -> Result<Option<u32>, ImportError> {
    let Some(value) = node.child_text("BaseCalendarUID") else {
        return Ok(None);
    };
    // MSPDI writes -1 (and, in some files, other negative numbers such as
    // -65533) for "no base calendar".
    match value.parse::<i64>() {
        Ok(uid) if uid < 0 => Ok(None),
        Ok(uid) => u32::try_from(uid)
            .map(Some)
            .map_err(|_| invalid("BaseCalendarUID", value)),
        Err(_) => Err(invalid("BaseCalendarUID", value)),
    }
}

/// One `WorkingTime` as `(from, to)` minutes, or `None` when `FromTime` or `ToTime` is missing,
/// does not parse, or does not fit `u16`.
fn working_time(time: &Node) -> Option<DayPeriod> {
    let from = parse_time_of_day(time.child_text("FromTime")?)?;
    let mut to = parse_time_of_day(time.child_text("ToTime")?)?;
    // A ToTime of 00:00 is the end of the day; 00:00 to 00:00 is a 24-hour working day.
    if to == 0 {
        to = 1440;
    }
    Some((u16::try_from(from).ok()?, u16::try_from(to).ok()?))
}

/// The working periods of a `WeekDay` or `Exception`, and how many `WorkingTime` elements
/// could not be read. The caller passes both to `keep_valid_periods`.
fn working_periods(node: &Node) -> (Vec<DayPeriod>, usize) {
    let mut periods = Vec::new();
    let mut unreadable = 0;
    if let Some(times) = node.child("WorkingTimes") {
        for time in times.children_named("WorkingTime") {
            match working_time(time) {
                Some(period) => periods.push(period),
                None => unreadable += 1,
            }
        }
    }
    periods.sort_unstable_by_key(|period| period.0);
    (periods, unreadable)
}

/// Drop the periods of one day that the engine would refuse (overlapping, empty or past
/// 24:00), as the XER import does, and the `unreadable` ones `working_periods` could not
/// parse, with one note naming the calendar and the day. The other periods of the day stay.
fn keep_valid_periods(
    periods: Vec<DayPeriod>,
    unreadable: usize,
    uid: u32,
    day: &str,
    notes: &mut Notes,
) -> Vec<DayPeriod> {
    let (kept, dropped) = keep_valid(periods);
    if dropped + unreadable > 0 {
        notes.line(format!(
            "Calendar {uid}: a working period on {day} was not read because it is invalid or overlaps another; the day's other periods are used, so its working hours can differ from the file's."
        ));
    }
    kept
}

/// The weekday an MSPDI `DayType` of 1 to 7 names (1 is Sunday).
fn weekday_name(day_type: u8) -> &'static str {
    const NAMES: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    NAMES
        .get(usize::from(day_type).saturating_sub(1))
        .copied()
        .unwrap_or("an unknown day")
}

fn parse_definition(
    node: &Node,
    origin_day: i64,
    notes: &mut Notes,
) -> Result<CalendarDefinition, ImportError> {
    let uid = required_u32(node, "UID")?;
    let mut week: [Option<Vec<DayPeriod>>; 7] = std::array::from_fn(|_| None);
    let mut found = Vec::new();

    if let Some(week_days) = node.child("WeekDays") {
        for week_day in week_days.children_named("WeekDay") {
            let day_value = required_text(week_day, "DayType")?;
            let day_type: u8 = day_value
                .parse()
                .map_err(|_| invalid("DayType", day_value))?;
            if day_type == 0 {
                found.extend(exception_days(week_day, uid, origin_day, notes)?);
                continue;
            }
            if !(1..=7).contains(&day_type) {
                return Err(invalid("DayType", day_value));
            }
            let index = usize::from((day_type + 5) % 7);
            let periods = match flag(week_day, "DayWorking")? {
                Some(true) => {
                    let (periods, unreadable) = working_periods(week_day);
                    Some(keep_valid_periods(
                        periods,
                        unreadable,
                        uid,
                        weekday_name(day_type),
                        notes,
                    ))
                }
                Some(false) => Some(Vec::new()),
                None => {
                    return Err(ImportError::Missing {
                        element: "DayWorking".to_owned(),
                    });
                }
            };
            let Some(day) = week.get_mut(index) else {
                return Err(invalid("DayType", day_value));
            };
            *day = periods;
        }
    }

    if let Some(exceptions) = node.child("Exceptions") {
        for exception in exceptions.children_named("Exception") {
            found.extend(exception_days(exception, uid, origin_day, notes)?);
        }
    }

    // A work week sets its dates, but an exception on a date beats it. `resolve` keeps a base
    // calendar's holiday over the work week's dates.
    let mut work_week = Vec::new();
    if let Some(work_weeks) = node.child("WorkWeeks") {
        let own: HashSet<i64> = found.iter().map(|(day, _)| *day).collect();
        for week in work_weeks.children_named("WorkWeek") {
            let days = work_week_days(week, uid, origin_day, notes)?;
            work_week.extend(days.into_iter().filter(|(day, _)| !own.contains(day)));
        }
    }

    Ok(CalendarDefinition {
        uid,
        is_base: flag(node, "IsBaseCalendar")?.unwrap_or(false),
        layer: Layer {
            base: base_uid(node)?,
            week,
            exceptions: dedupe(found, uid, notes),
            work_week: dedupe(work_week, uid, notes),
        },
    })
}

fn project_calendar_uid(root: &Node) -> Result<Option<u32>, ImportError> {
    let Some(value) = root.child_text("CalendarUID") else {
        return Ok(None);
    };
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| invalid("CalendarUID", value))
}

/// The note for a problem in a calendar's base chain.
fn chain_note(warning: &ChainWarning) -> String {
    match warning {
        ChainWarning::BaseMissing { uid, base } => {
            format!(
                "Calendar {uid}: its base calendar {base} is not in the file, so the base's working time is not used; its working days can differ from the file's."
            )
        }
        ChainWarning::Cycle { uid } => format!(
            "Calendar {uid}: its base calendars form a loop, so the loop was not followed; its working days can differ from the file's."
        ),
    }
}

/// The calendars read so far, in document order, waiting for [`CalendarSet::build`].
pub(crate) struct CalendarSet {
    requested_default: Option<u32>,
    definitions: Vec<CalendarDefinition>,
    indexes: HashMap<u32, usize>,
}

impl CalendarSet {
    /// An empty set; `root` is the project element, whose `CalendarUID` names the default.
    pub(crate) fn new(root: &Node) -> Result<CalendarSet, ImportError> {
        Ok(CalendarSet {
            requested_default: project_calendar_uid(root)?,
            definitions: Vec::new(),
            indexes: HashMap::new(),
        })
    }

    /// Read one `Calendar` element.
    pub(crate) fn add(
        &mut self,
        calendar: &Node,
        origin_day: i64,
        notes: &mut Notes,
    ) -> Result<(), ImportError> {
        let uid_value = required_text(calendar, "UID")?;
        let definition = parse_definition(calendar, origin_day, notes)?;
        if self
            .indexes
            .insert(definition.uid, self.definitions.len())
            .is_some()
        {
            return Err(invalid("UID", uid_value));
        }
        self.definitions.push(definition);
        Ok(())
    }

    /// Build every calendar added, in document order, and choose the project default.
    pub(crate) fn build(
        self,
        schedule: &mut Schedule,
        notes: &mut Notes,
    ) -> Result<Calendars, ImportError> {
        build(self, schedule, notes)
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "resolves every definition in one linear pass; splitting it would only move the same locals into arguments"
)]
fn build(
    set: CalendarSet,
    schedule: &mut Schedule,
    notes: &mut Notes,
) -> Result<Calendars, ImportError> {
    let CalendarSet {
        requested_default,
        definitions,
        indexes,
    } = set;
    let mut by_uid = HashMap::new();
    for definition in &definitions {
        let resolved = calendar_rule::resolve(
            definition.uid,
            |uid| {
                indexes
                    .get(&uid)
                    .and_then(|&position| definitions.get(position))
                    .map(|found| &found.layer)
            },
            Holidays::Inherit,
        )?;
        for warning in &resolved.warnings {
            notes.line(chain_note(warning));
        }
        let calendar = match Calendar::with_exceptions(resolved.week, &resolved.exceptions) {
            Ok(calendar) => calendar,
            // Resource calendars often list no working time; a task that
            // names one gets the project calendar.
            Err(CalendarError::NoWorkingTime) => {
                notes.line(format!(
                    "Calendar {}: it has no working time and was not imported; tasks that use it get the project calendar, so their dates can differ from the file's.",
                    definition.uid
                ));
                continue;
            }
            Err(source) => {
                return Err(ImportError::Calendar {
                    uid: definition.uid,
                    source,
                });
            }
        };
        let id = schedule.add_calendar(calendar);
        by_uid.insert(definition.uid, id);
    }

    let built = |base_only: bool| {
        definitions
            .iter()
            .filter(|definition| by_uid.contains_key(&definition.uid))
            .find(|definition| !base_only || definition.is_base)
    };
    let Some(first_definition) = built(false) else {
        if let Some(uid) = requested_default {
            notes.line(format!(
                "Project: calendar {uid} is not in the file, so the default calendar is used and dates can differ from the file's."
            ));
        }
        let fallback = Calendar::new(calendar_rule::fallback_week(), &[])
            .map_err(|source| ImportError::Calendar { uid: 0, source })?;
        let default = schedule.add_calendar(fallback);
        notes.line(
            "Project: the file has no calendars, so Monday to Friday 08:00 to 17:00 is used and dates can differ from the file's."
                .to_owned(),
        );
        return Ok(Calendars::new(default, by_uid));
    };
    let absent_default_definition = built(true).unwrap_or(first_definition);
    let lookup = |uid: u32| {
        by_uid
            .get(&uid)
            .copied()
            .ok_or_else(|| ImportError::Missing {
                element: "Calendar".to_owned(),
            })
    };
    let default = match requested_default {
        Some(uid) => {
            if let Some(calendar) = by_uid.get(&uid).copied() {
                calendar
            } else {
                notes.line(format!(
                    "Project: calendar {uid} is not in the file, so calendar {} is used for the project and dates can differ from the file's.",
                    first_definition.uid
                ));
                lookup(first_definition.uid)?
            }
        }
        None => lookup(absent_default_definition.uid)?,
    };

    Ok(Calendars::new(default, by_uid))
}

#[cfg(test)]
mod tests;
