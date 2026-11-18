//! The Gantt scheduling engine (milestone M1).
//!
//! Pure Rust, no WebAssembly dependencies: the model ([`Schedule`]), working
//! calendars ([`Calendar`]), a critical-path forward and backward pass that
//! finds every bound by search ([`Schedule::compute`]), and a deterministic
//! schedule generator ([`generate()`], [`stress`]).
//!
//! Every date is an [`Instant`]: integer wall-clock minutes from the project
//! origin, which is a Monday 00:00. Durations and lags are working minutes
//! of a calendar and exist only inside calendar math.

#![cfg_attr(not(test), warn(clippy::too_many_lines))]

pub mod calendar;
pub mod cpm;
mod diff;
pub mod generate;
mod links;
pub mod model;
mod topo;

pub use calendar::{Calendar, CalendarError, DAY, DayPeriod, Instant, Minutes, WEEK};
/// The duration cone's test seams (gs#58); only with the `test-hooks` feature.
#[cfg(feature = "test-hooks")]
pub use cpm::duration::seam as test_hooks;
pub use cpm::duration::{DurationCone, FallbackReason};
pub use cpm::{Admissible, Moved, Solution, TaskDates};
pub use diff::{SolutionDiff, TaskChange, diff};
pub use generate::{Rng, generate, generate_demo, stress, stress_with_exceptions};
pub use model::options::{
    DEFAULT_STARTED_LATE_START, DEFAULT_UNSTARTED_REMAINING, ScheduleOptions, StartedLateStart,
    UnstartedRemaining,
};
pub use model::{
    CalendarId, DepId, Dependency, LagCalendar, Limit, LinkType, MAX_DATE, MAX_DURATION, MAX_LAG,
    MIN_DATE, Milestone, Pin, RangeInput, RangeItem, Schedule, ScheduleError, Task, TaskId,
    finish_by::Buffer,
};
