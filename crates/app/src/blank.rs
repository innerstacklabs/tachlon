//! A schedule with no tasks (gs#703): the start of a new plan.

use crate::dates::Origin;
use crate::outline::Outline;
use crate::{Loaded, OpenTimings};
use engine::{Calendar, Schedule};

/// 1970-01-01 was a Thursday, three days after a Monday: adding it to a day count makes
/// day 0 of the week a Monday.
const THURSDAY: i64 = 3;

impl Loaded {
    /// A solved schedule with one Monday-to-Friday calendar (08:00-12:00 and 13:00-17:00, 480
    /// minutes a day) and no tasks. Its origin is the Monday on or before `start_day`, a day count
    /// since 1970-01-01. An insert on a session over it makes the first row.
    ///
    /// # Panics
    /// Never: the calendar, the solve and the outline of an empty schedule all succeed.
    #[must_use]
    pub fn blank(start_day: i64) -> Loaded {
        let day = vec![(480, 720), (780, 1020)];
        let week =
            std::array::from_fn(|weekday| if weekday < 5 { day.clone() } else { Vec::new() });
        let mut schedule = Schedule::new();
        schedule.add_calendar(Calendar::new(week, &[]).expect("a Monday-to-Friday week is valid"));
        let solution = schedule.compute().expect("a schedule with no tasks solves");
        let outline = Outline::new(&[], &[], &schedule).expect("no tasks make an empty outline");
        Loaded {
            schedule,
            solution,
            order: Vec::new(),
            meta: Vec::new(),
            outline,
            unsupported: Vec::new(),
            data_date: None,
            origin: Origin::from_day(start_day - (start_day + THURSDAY).rem_euclid(7)),
            progress: Vec::new(),
            deadlines: Vec::new(),
            resources: Vec::new(),
            baseline: None,
            timings: OpenTimings::default(),
        }
    }
}
