//! Whose working time a dependency's lag is counted in: the schedule's choice and the
//! per-dependency override (gs#692).

use super::{DepId, Schedule, ScheduleError};
use crate::calendar::{Calendar, DAY, DayPeriod};

/// Whose working time a dependency's lag is counted in. One choice per
/// schedule ([`Schedule::set_lag_calendar`]), which a single dependency can
/// override ([`Schedule::set_dependency_lag_calendar`]); the default is `Successor`.
///
/// Every choice resolves to one [`Calendar`] and the lag goes through the
/// same [`Calendar::add`], so the soundness argument of the passes (`add` is
/// monotone in the instant) holds for each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LagCalendar {
    /// The successor task's calendar.
    #[default]
    Successor,
    /// The predecessor task's calendar.
    Predecessor,
    /// A calendar that works every minute: a lag of 1440 is one elapsed day.
    TwentyFourHour,
    /// One named calendar of the schedule, for every dependency. An importer
    /// that means "the project default calendar" passes that calendar's id.
    Fixed(super::CalendarId),
}

impl Schedule {
    /// Refuse a `Fixed` id with no calendar; build the all-minutes calendar
    /// the first time `TwentyFourHour` is chosen.
    fn prepare_lag_calendar(&mut self, lag: LagCalendar) -> Result<(), ScheduleError> {
        match lag {
            LagCalendar::Fixed(id) if id.0 as usize >= self.calendars.len() => {
                return Err(ScheduleError::UnknownCalendar(id));
            }
            LagCalendar::TwentyFourHour if self.all_day.is_none() => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "DAY is the constant 1440, below u16::MAX"
                )]
                let week: [Vec<DayPeriod>; 7] = std::array::from_fn(|_| vec![(0, DAY as u16)]);
                self.all_day = Some(Calendar::new(week, &[]).expect("an all-minutes calendar"));
            }
            _ => {}
        }
        Ok(())
    }

    /// Choose the calendar every dependency's lag is counted in, unless the
    /// dependency has its own ([`Schedule::set_dependency_lag_calendar`]). The
    /// default is [`LagCalendar::Successor`]. Keeps the cached topological
    /// order: no dependency changes. A `Fixed` id with no calendar is refused
    /// with [`ScheduleError::UnknownCalendar`] and nothing changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownCalendar`] for a `Fixed` id with no calendar.
    pub fn set_lag_calendar(&mut self, lag: LagCalendar) -> Result<(), ScheduleError> {
        self.prepare_lag_calendar(lag)?;
        self.lag_calendar = lag;
        Ok(())
    }

    /// The calendar choice for lags.
    #[must_use]
    pub fn lag_calendar(&self) -> LagCalendar {
        self.lag_calendar
    }

    /// Count one dependency's lag in `lag`'s calendar instead of the
    /// schedule's choice; `None` goes back to the schedule's choice. Keeps the
    /// cached topological order. An id with no dependency is refused with
    /// [`ScheduleError::UnknownDependency`], a `Fixed` id with no calendar with
    /// [`ScheduleError::UnknownCalendar`]; either way nothing changes. A removed
    /// dependency keeps its setting, so an undo brings it back. Solve again
    /// after a change: a stored [`crate::Solution`] still holds the old dates.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] for an id with no dependency and
    /// [`ScheduleError::UnknownCalendar`] for a `Fixed` id with no calendar.
    pub fn set_dependency_lag_calendar(
        &mut self,
        dep: DepId,
        lag: Option<LagCalendar>,
    ) -> Result<(), ScheduleError> {
        if dep.0 as usize >= self.deps.len() {
            return Err(ScheduleError::UnknownDependency(dep));
        }
        if let Some(lag) = lag {
            self.prepare_lag_calendar(lag)?;
        }
        if self.lag_over.len() <= dep.0 as usize {
            self.lag_over.resize(self.deps.len(), None);
        }
        self.lag_over[dep.0 as usize] = lag;
        Ok(())
    }

    /// The dependency's own lag calendar, or `None` when it follows the
    /// schedule's choice ([`Schedule::lag_calendar`]).
    #[must_use]
    pub fn dependency_lag_calendar(&self, dep: DepId) -> Option<LagCalendar> {
        self.lag_over.get(dep.0 as usize).copied().flatten()
    }
}
