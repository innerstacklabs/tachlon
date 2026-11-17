//! The calendar a lag is counted in, resolved against one schedule.

use crate::calendar::Calendar;
use crate::model::{Dependency, LagCalendar, Task};

/// A [`LagCalendar`] resolved against one schedule: the calendar a lag is
/// counted in is either read off the dependency or one calendar shared by all.
#[derive(Clone, Copy)]
pub(super) enum Choice<'a> {
    Successor,
    Predecessor,
    Shared(&'a Calendar),
}

impl<'a> Choice<'a> {
    pub(super) fn resolve(
        lag: LagCalendar,
        cals: &'a [Calendar],
        all_day: Option<&'a Calendar>,
    ) -> Self {
        match lag {
            LagCalendar::Successor => Choice::Successor,
            LagCalendar::Predecessor => Choice::Predecessor,
            LagCalendar::TwentyFourHour => Choice::Shared(all_day.expect("set with the choice")),
            LagCalendar::Fixed(id) => Choice::Shared(&cals[id.0 as usize]),
        }
    }
}

/// The schedule's lag calendar, and the dependencies that count their lag in
/// a calendar of their own (`own[i]`, for dependency `i`; may be shorter than
/// the dependencies).
#[derive(Clone, Copy)]
pub(in crate::cpm) struct LagSource<'a> {
    pub(super) default: Choice<'a>,
    pub(super) own: &'a [Option<LagCalendar>],
    pub(super) all_day: Option<&'a Calendar>,
}

impl<'a> LagSource<'a> {
    /// The calendar the lag of dependency `di`, `d`, is counted in.
    #[inline]
    pub(in crate::cpm) fn cal(
        self,
        cals: &'a [Calendar],
        tasks: &[Task],
        di: usize,
        d: &Dependency,
    ) -> &'a Calendar {
        let choice = match self.own.get(di) {
            Some(Some(own)) => Choice::resolve(*own, cals, self.all_day),
            _ => self.default,
        };
        match choice {
            Choice::Successor => &cals[tasks[d.succ.0 as usize].calendar.0 as usize],
            Choice::Predecessor => &cals[tasks[d.pred.0 as usize].calendar.0 as usize],
            Choice::Shared(c) => c,
        }
    }
}
