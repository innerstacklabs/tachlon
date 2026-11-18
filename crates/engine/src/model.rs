//! Schedule model: tasks, dependencies, calendars, and the cached
//! topological order (plan D6: cached until the dependency set changes).

use std::fmt;

use crate::calendar::{Calendar, Instant, Minutes};
use crate::topo;

mod error;
pub mod finish_by;
mod lag;
mod limit;
pub mod options;
mod progress;
pub(crate) mod range;

pub use error::ScheduleError;
pub use lag::LagCalendar;
pub use limit::{Limit, Milestone, Pin};
pub use range::{MAX_DATE, MAX_DURATION, MAX_LAG, MIN_DATE, RangeInput, RangeItem};

/// Index of a task in its [`Schedule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub u32);

/// Index of a calendar in its [`Schedule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CalendarId(pub u32);

/// Index of a dependency in its [`Schedule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepId(pub u32);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task {}", self.0)
    }
}

impl fmt::Display for DepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dependency {}", self.0)
    }
}

/// Dependency type: which predecessor date drives which successor date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkType {
    /// Finish-to-start.
    FS,
    /// Start-to-start.
    SS,
    /// Finish-to-finish.
    FF,
    /// Start-to-finish.
    SF,
}

/// A task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Duration in working minutes of `calendar`. Zero makes a milestone.
    pub duration: Minutes,
    /// The task's calendar.
    pub calendar: CalendarId,
    /// Start-no-earlier-than constraint.
    pub snet: Option<Instant>,
    /// The late-start, early-finish and late-finish limits ([`Limit`]).
    pub limit: Limit,
    /// A tombstone: the task keeps its id and its slot, but it is not part of
    /// the schedule. Set by [`Schedule::delete_task`], cleared by
    /// [`Schedule::restore_task`]. A dependency with a deleted end is dead too
    /// (derived, not stored).
    pub deleted: bool,
    /// When work on the task actually began. Pins the early start: the bar starts here, whatever
    /// its predecessors, start-no-earlier-than or the data date say (decision 0008). A date
    /// before the origin acts as the origin. `None`: not started.
    pub actual_start: Option<Instant>,
    /// When work on the task actually ended. A task with an actual finish is fixed at its
    /// actuals (a finish without an actual start is a bar of no length at the finish) and its
    /// float is 0. `None`: not finished.
    pub actual_finish: Option<Instant>,
    /// Working minutes of `calendar` still to do, once the task has started: the remaining work
    /// runs from the data date (or later, after its predecessors) for this long. `None`: the
    /// whole `duration` remains. Ignored until `actual_start` is set. Percent complete is derived
    /// from it, never stored (decision 0008, pick 1).
    pub remaining: Option<Minutes>,
}

/// A dependency of `succ` on `pred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependency {
    /// Predecessor.
    pub pred: TaskId,
    /// Successor.
    pub succ: TaskId,
    /// Link type.
    pub kind: LinkType,
    /// Lag in working minutes of the dependency's lag calendar
    /// ([`Schedule::dependency_lag_calendar`], else [`Schedule::lag_calendar`]); may be negative.
    pub lag: Minutes,
}

/// Adjacency derived from the dependency set, plus a topological order.
#[derive(Debug, Clone)]
pub(crate) struct Graph {
    pub order: Vec<u32>,
    /// `rank[i]`: where task `i` sits in `order` (`u32::MAX` for a deleted task).
    /// Built with the order, so a cone pass sorts its work without scanning `order`.
    pub rank: Vec<u32>,
    /// CSR: dependencies into task `i` are `pred_deps[pred_start[i]..pred_start[i + 1]]`.
    pub pred_start: Vec<u32>,
    pub pred_deps: Vec<u32>,
    /// CSR: dependencies out of task `i`.
    pub succ_start: Vec<u32>,
    pub succ_deps: Vec<u32>,
}

/// Tasks, dependencies and calendars.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    pub(crate) calendars: Vec<Calendar>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) deps: Vec<Dependency>,
    /// `removed[i]`: dependency `i` was taken out by a person
    /// ([`Schedule::remove_dependency`]). Same length as `deps`; the slot and
    /// its [`DepId`] stay.
    pub(crate) removed: Vec<bool>,
    pub(crate) graph: Option<Graph>,
    pub(crate) lag_calendar: LagCalendar,
    /// `lag_over[i]`: dependency `i` counts its lag in its own calendar. May be shorter than
    /// `deps` (empty until the first override); a missing slot follows `lag_calendar`.
    pub(crate) lag_over: Vec<Option<LagCalendar>>,
    /// The all-minutes calendar `LagCalendar::TwentyFourHour` resolves to,
    /// built when that choice is first set.
    pub(crate) all_day: Option<Calendar>,
    /// The data date: the instant "today" is, for progress. Unstarted work starts no earlier
    /// ([`Schedule::set_data_date`]). `None`: no data date, the origin.
    pub(crate) data_date: Option<Instant>,
    /// Some task has carried a [`Limit`]: a late limit no date can meet then shows as negative
    /// float instead of an engine error. Never cleared.
    pub(crate) limited: bool,
    pub(crate) finish_by: Option<Instant>,
    pub(crate) options: options::ScheduleOptions,
}

impl Schedule {
    /// An empty schedule.
    #[must_use]
    pub fn new() -> Schedule {
        Schedule::default()
    }

    /// Add a calendar.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the calendar count is not checked against u32::MAX, so the id wraps past 2^32 - \
                  1 calendars (unchecked)"
    )]
    pub fn add_calendar(&mut self, cal: Calendar) -> CalendarId {
        self.calendars.push(cal);
        CalendarId(self.calendars.len() as u32 - 1)
    }

    /// Make room for `more` tasks, if the memory can be had; a reader that can guess the count
    /// calls this, then [`Schedule::shrink_tasks`] once the tasks are in.
    pub fn reserve_tasks(&mut self, more: usize) {
        let _ = self.tasks.try_reserve_exact(more);
    }

    /// Give back the room [`Schedule::reserve_tasks`] kept and the tasks did not use.
    pub fn shrink_tasks(&mut self) {
        self.tasks.shrink_to_fit();
    }

    /// Add a task.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownCalendar`] if the task's calendar does not exist and
    /// [`ScheduleError::NegativeDuration`] if its duration or remaining duration is negative and
    /// [`ScheduleError::OutOfRange`] if a duration or a date, a limit and a pin included, is out of
    /// range.
    pub fn add_task(&mut self, task: Task) -> Result<TaskId, ScheduleError> {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the task count is not checked against u32::MAX, so the id wraps past 2^32 - \
                      1 tasks (unchecked)"
        )]
        let id = TaskId(self.tasks.len() as u32);
        if task.calendar.0 as usize >= self.calendars.len() {
            return Err(ScheduleError::UnknownCalendar(task.calendar));
        }
        range::check_task(id, &task)?;
        self.limited |= task.limit != Limit::default();
        self.tasks.push(task);
        self.graph = None;
        Ok(id)
    }

    /// Add a dependency, unchecked for loops and duplicates (importers; a
    /// loop shows at [`Schedule::compute`]). People use [`Schedule::add_link`].
    /// Invalidates the cached topological order.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted end and
    /// [`ScheduleError::SelfDependency`] if `dep.pred == dep.succ` and [`ScheduleError::OutOfRange`]
    /// if the lag is beyond [`MAX_LAG`] either way.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the dependency count is not checked against u32::MAX, so the id wraps past 2^32 \
                  - 1 dependencies (unchecked)"
    )]
    pub fn add_dependency(&mut self, dep: Dependency) -> Result<DepId, ScheduleError> {
        for t in [dep.pred, dep.succ] {
            self.live_mut(t)?;
        }
        if dep.pred == dep.succ {
            return Err(ScheduleError::SelfDependency(dep.pred));
        }
        let id = DepId(self.deps.len() as u32);
        range::check_lag(id, dep.lag)?;
        self.deps.push(dep);
        self.removed.push(false);
        self.graph = None;
        Ok(id)
    }

    /// Set or clear a start-no-earlier-than constraint. Keeps the cached
    /// topological order: a drag changes only a constraint.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task and
    /// [`ScheduleError::OutOfRange`] for a date out of range.
    pub fn set_snet(&mut self, task: TaskId, snet: Option<Instant>) -> Result<(), ScheduleError> {
        let t = self.live_mut(task)?;
        range::check_date(RangeInput::Snet, RangeItem::Task(task), snet)?;
        t.snet = snet;
        Ok(())
    }

    /// Set or clear the data date. Work that has not started cannot start before it: the forward
    /// pass floors every such task's early start at the data date, as a start-no-earlier-than
    /// would. A date before the origin acts as the origin. Keeps the cached topological order.
    ///
    /// The data date reaches every unstarted task, so the cones
    /// ([`Schedule::recompute_cone`], [`Schedule::recompute_duration_cone_moved`]) cannot bring a
    /// solution up to date after it changes: call [`Schedule::compute`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::OutOfRange`] for a date out of range, and nothing changes.
    pub fn set_data_date(&mut self, data_date: Option<Instant>) -> Result<(), ScheduleError> {
        range::check_date(RangeInput::DataDate, RangeItem::Schedule, data_date)?;
        self.data_date = data_date;
        Ok(())
    }

    /// Set a task's duration, in working minutes of its calendar (zero makes
    /// a milestone). Keeps the cached topological order: only a task field
    /// changes. A negative duration or one over [`MAX_DURATION`] is refused and nothing changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted task,
    /// [`ScheduleError::NegativeDuration`] for a negative duration and
    /// [`ScheduleError::OutOfRange`] for one over [`MAX_DURATION`].
    pub fn set_duration(&mut self, task: TaskId, duration: Minutes) -> Result<(), ScheduleError> {
        let t = self.live_mut(task)?;
        range::check_duration(RangeInput::Duration, task, duration)?;
        t.duration = duration;
        Ok(())
    }

    /// The task, if it exists and is not deleted: a deleted task is unknown to
    /// every operation.
    fn live_mut(&mut self, task: TaskId) -> Result<&mut Task, ScheduleError> {
        match self.tasks.get_mut(task.0 as usize) {
            Some(t) if !t.deleted => Ok(t),
            _ => Err(ScheduleError::UnknownTask(task)),
        }
    }

    /// Tombstone a task. It keeps its id and slot; every dependency touching
    /// it stops applying; its [`Solution`](crate::Solution) slot reads zero.
    /// A task that is unknown or already deleted is refused with
    /// [`ScheduleError::UnknownTask`] and nothing changes. Invalidates the
    /// cached topological order.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or already deleted task.
    pub fn delete_task(&mut self, task: TaskId) -> Result<(), ScheduleError> {
        self.live_mut(task)?.deleted = true;
        self.graph = None;
        Ok(())
    }

    /// Bring a deleted task back with its fields and its dependencies as they
    /// were. Each link that comes back into force is checked like
    /// [`Schedule::restore_dependency`]: one that would repeat a live link or
    /// close a loop is refused ([`ScheduleError::DuplicateDependency`],
    /// [`ScheduleError::Cycle`]) and nothing changes. A live task is left as
    /// it is; an unknown id is refused with [`ScheduleError::UnknownTask`].
    /// Invalidates the cached topological order when it changes anything.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown id, and
    /// [`ScheduleError::DuplicateDependency`] or [`ScheduleError::Cycle`] if a link that comes back
    /// would repeat a live one or close a loop.
    pub fn restore_task(&mut self, task: TaskId) -> Result<(), ScheduleError> {
        match self.tasks.get(task.0 as usize) {
            None => Err(ScheduleError::UnknownTask(task)),
            Some(t) if t.deleted => self.restore_checked(task),
            Some(_) => Ok(()),
        }
    }

    /// Is `task` a tombstone? An unknown id is not.
    #[must_use]
    pub fn is_deleted(&self, task: TaskId) -> bool {
        self.tasks.get(task.0 as usize).is_some_and(|t| t.deleted)
    }

    /// The number of tasks that are not deleted.
    #[must_use]
    pub fn live_task_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.deleted).count()
    }

    /// Is the dependency in force: not removed, and neither end deleted?
    #[must_use]
    pub fn is_dependency_live(&self, dep: DepId) -> bool {
        (dep.0 as usize) < self.deps.len() && self.live_at(dep.0 as usize)
    }

    /// [`Schedule::is_dependency_live`] by slot index, which must be in range.
    pub(crate) fn live_at(&self, i: usize) -> bool {
        let d = &self.deps[i];
        !self.removed[i]
            && !self.tasks[d.pred.0 as usize].deleted
            && !self.tasks[d.succ.0 as usize].deleted
    }

    /// The calendars.
    #[must_use]
    pub fn calendars(&self) -> &[Calendar] {
        &self.calendars
    }

    /// The tasks.
    #[must_use]
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    /// The dependency slots, removed and dead ones included; the index is the
    /// [`DepId`]. [`Schedule::live_dependencies`] lists those in force.
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        &self.deps
    }

    /// The calendar of a task.
    #[must_use]
    pub fn calendar_of(&self, task: TaskId) -> &Calendar {
        &self.calendars[self.tasks[task.0 as usize].calendar.0 as usize]
    }

    /// Is the topological order cached?
    #[must_use]
    pub fn has_cached_order(&self) -> bool {
        self.graph.is_some()
    }

    /// Drop the cached order (benchmarks measure a cold compute with it).
    pub fn clear_cached_order(&mut self) {
        self.graph = None;
    }

    /// The topological order (Kahn), computed and cached on first use.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::Cycle`] if the dependencies contain a loop.
    pub fn topological_order(&mut self) -> Result<Vec<TaskId>, ScheduleError> {
        Ok(self.graph()?.order.iter().map(|&i| TaskId(i)).collect())
    }

    pub(crate) fn graph(&mut self) -> Result<&Graph, ScheduleError> {
        if self.graph.is_none() {
            self.graph = Some(topo::build(&self.tasks, &self.deps, &self.removed)?);
        }
        Ok(self.graph.as_ref().expect("just built"))
    }
}
