//! [`Session`]: the schedule, its solution and the timeline, with the pointer input that moves a bar.

use crate::meta::TaskMeta;
use crate::outline::{Outline, OutlineError, RollupScratch};
use crate::press::{LinkState, Press};
use crate::view::shading_calendar;
use crate::{
    DragInfo, RowFilter, Selection, dates, fitted_timeline, history, lookahead, moved, range,
};
use engine::{Moved, Schedule, Solution, TaskId};
use timeline::scene::LinkIndex;
use timeline::{Summaries, Theme, Timeline};

/// The schedule, its solution and the timeline, with the pointer input
/// that moves a bar. Plain Rust, so the drag path is tested natively; the
/// wasm `App` owns one and adds the canvas and the renderer.
///
/// Invariant: `solution` is the solution of `schedule`, and the timeline's summary spans are the
/// rollup of that solution. The engine's cone recompute trusts the solution, so stale dates give
/// wrong results with no error. These fields are private to the crate, and edits rebuild summaries
/// on every path that changes the solution through [`Session::apply`] (with `constrain` as the
/// drag wrapper), so nothing outside `Session` can break the invariant; readers use
/// [`Session::schedule`] and [`Session::solution`].
#[must_use]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the flags are independent facts: a pending fit, stale rows, a stale selection, a started drag"
)]
pub struct Session {
    pub(crate) schedule: Schedule,
    pub(crate) solution: Solution,
    pub(crate) timeline: Timeline,
    pub(crate) theme: Theme,
    /// The calendar whose non-working days the scene shades.
    pub(crate) shading: Option<engine::CalendarId>,
    /// The data date the scene marks.
    pub(crate) data_date: Option<engine::Instant>,
    /// The calendar date of instant 0; [`Session::date_text`] and its siblings write dates from it.
    pub(crate) origin: dates::Origin,
    pub(crate) outline: Option<Outline>,
    /// The view is still the opening fit: a canvas that changes width refits it (the ID column appears after an open), until the first wheel or fit.
    pub(crate) fit_pending: bool,
    pub(crate) moved: Vec<Moved>,
    pub(crate) reports: moved::Reports,
    pub(crate) rollup_scratch: RollupScratch,
    pub(crate) collapsed: Vec<bool>,
    /// Which leaf predicate, if any, limits the visible outline rows.
    pub(crate) row_filter: RowFilter,
    /// The lookahead window, if one is set. It combines with `row_filter`: a task has to pass both.
    pub(crate) lookahead: Option<lookahead::Lookahead>,
    /// The grouping by an activity code type, if one is set (decision 0015).
    pub(crate) group: Option<crate::group::Grouping>,
    /// A solution change happened during a press and the filtered rows need
    /// refreshing when the press ends.
    pub(crate) rows_stale: bool,
    /// The leaf tasks the row filter and the lookahead window keep, as of the last row rebuild;
    /// `None` while neither is on.
    pub(crate) kept_leaves: Option<u32>,
    /// The longest path's line, as of the last row rebuild; `None` unless Longest path is on.
    pub(crate) path_line: Option<crate::path_line::PathLine>,
    pub(crate) links: LinkState,
    /// Counts the changes the schedule has had. A drag that ends where it
    /// began, or is cancelled, puts the schedule back and leaves this at
    /// the value it had at the press. The table keys on `stamp`, not this.
    pub(crate) revision: u32,
    /// Counts the changes the table has to redraw for; the table redraws
    /// whenever it changes. It goes up on a new solution (an edit), a baseline set or cleared, on a
    /// drag's restore when the ask was never drawn, when an outline is set
    /// (`with_outline`), on `reopen` (which starts past this session's), and
    /// on a row change: a toggle, a new row filter or lookahead window, a
    /// level command that changes the rows or the collapsed flags, and any
    /// `apply_rows` that changes the visible rows. It only goes up: a
    /// restore is a new solution to draw even though `revision` goes back.
    pub(crate) stamp: u32,
    /// Last drag result, for the overlay.
    pub(crate) drag_info: Option<DragInfo>,
    /// Number of driving-path cache builds.
    pub(crate) chain_builds: u32,
    pub(crate) reveal: crate::reveal::Reveal,
    pub(crate) selection: Option<Selection>,
    pub(crate) selection_stale: bool,
    /// The range selection (gs#423): the selected task is its anchor, and the extent is the other end.
    pub(crate) range: Option<range::Range>,
    /// The last drag applied: (task, wanted start). A frame recomputes only
    /// when the drag asks for something new, so many pointer moves between
    /// two frames cost one recompute.
    pub(crate) applied: Option<(u32, i64)>,
    /// The press on a bar, until it is released or cancelled.
    pub(crate) press: Option<Press>,
    /// A press on a finished task's bar: its row and where it landed. It is no [`Press`], so no
    /// drag frame edits anything; the release says why the bar did not move.
    pub(crate) finished_press: Option<(u32, (f64, f64))>,
    /// The pointer has moved [`DRAG_THRESHOLD_PX`] from `press`: the press
    /// is a drag. Stays set until the release, even if the pointer comes back.
    pub(crate) drag_started: bool,
    /// Last pointer position of a bar press, for the hover row.
    pub(crate) pointer: (f64, f64),
    /// Milliseconds clock for `DragInfo::recompute_ms` (`performance.now`
    /// in the browser).
    pub(crate) clock: fn() -> f64,
    /// Press point, press time, and task under the pointer (if any).
    pub(crate) click: Option<(f64, f64, f64, Option<u32>)>,
    /// Undo and redo stacks.
    pub(crate) history: history::History,
    /// The changes a program proposed, held until the person accepts or rejects them all (gantt#1250); `reopen` starts without one.
    pub(crate) proposal: Option<crate::proposal::Proposal>,
    /// App-owned display data for each task, indexed like [`Schedule::tasks`]. A rename changes it.
    pub(crate) meta: Vec<TaskMeta>,
    /// Task ids from the most recent [`Session::find`], in display order.
    pub(crate) found: Vec<u32>,
    /// The baseline a person set (gs#703); `reopen` starts without one.
    pub(crate) baseline: Option<std::sync::Arc<crate::baseline::Baseline>>,
    /// The percent typed per task id; shown only while the remaining still is what it set.
    pub(crate) typed_percent: std::collections::HashMap<u32, u8>,
    /// What the host API keeps (see [`crate::host`]); `reopen` starts it empty but for `read_only`, `before_edit` and `capabilities`.
    pub(crate) host: crate::host::Host,
    /// The summary percents, kept per `(revision, stamp)` (gs#703); every new session starts empty.
    pub(crate) progress_rollup: crate::progress_rollup::Memo,
    /// The live links into each task, kept per `(revision, stamp)` for the predecessors read; every new session starts empty.
    pub(crate) predecessors: crate::edit::PredecessorsMemo,
    /// The MS Project deadline per task id, display only (`with_deadlines`); `reopen` starts empty.
    pub(crate) deadlines: Vec<Option<engine::Instant>>,
    /// The resource names per task id, display only (`with_resources`); `reopen` starts empty.
    pub(crate) resources: Vec<Vec<crate::Assigned>>,
    /// The month names the app's date labels use (`set_locale`); `reopen` carries them on.
    pub(crate) names: timeline::names::Names,
    /// The sentences this chart refuses an edit in (`set_locale`); `reopen` carries them on.
    pub(crate) texts: crate::texts::Texts,
    /// The table shows the free float column ([`Session::set_free_float_shown`]); `reopen` carries it on.
    pub(crate) free_float_shown: bool,
}

impl Session {
    /// A session over `schedule` and `solution`. `solution` must be the
    /// solution of `schedule` (what [`Schedule::compute`] returned for it). `meta` is the display
    /// data of its tasks.
    pub fn new(
        schedule: Schedule,
        solution: Solution,
        timeline: Timeline,
        meta: Vec<TaskMeta>,
        clock: fn() -> f64,
    ) -> Session {
        let links = LinkIndex::of(&schedule, timeline.order());
        Session {
            shading: shading_calendar(&schedule),
            data_date: None,
            origin: dates::Origin::DEMO,
            schedule,
            solution,
            timeline,
            theme: Theme::default(),
            outline: None,
            fit_pending: true,
            moved: Vec::new(),
            reports: moved::Reports::default(),
            rollup_scratch: RollupScratch::default(),
            collapsed: Vec::new(),
            row_filter: RowFilter::Off,
            lookahead: None,
            group: None,
            free_float_shown: false,
            rows_stale: false,
            kept_leaves: None,
            path_line: None,
            links: LinkState::new(links),
            revision: 0,
            stamp: 0,
            drag_info: None,
            chain_builds: 0,
            reveal: crate::reveal::Reveal::default(),
            selection: None,
            selection_stale: false,
            range: None,
            applied: None,
            press: None,
            finished_press: None,
            drag_started: false,
            pointer: (0.0, 0.0),
            clock,
            click: None,
            history: history::History::new(history::HISTORY_CAP),
            proposal: None,
            meta,
            found: Vec::new(),
            baseline: None,
            typed_percent: std::collections::HashMap::new(),
            host: crate::host::Host::default(),
            progress_rollup: crate::progress_rollup::Memo::default(),
            predecessors: crate::edit::PredecessorsMemo::default(),
            deadlines: Vec::new(),
            resources: Vec::new(),
            names: timeline::names::Names::default(),
            texts: crate::texts::Texts::default(),
        }
    }

    /// Mark `date` (an instant, as `Loaded::data_date`) with the data-date line.
    /// `None` draws no line.
    pub fn with_data_date(mut self, date: Option<engine::Instant>) -> Session {
        self.data_date = date;
        self
    }

    /// The data date the scene marks, if the file had one.
    pub fn data_date(&self) -> Option<engine::Instant> {
        self.data_date
    }

    /// Set the calendar date of instant 0 (as `Loaded::origin`). A session that never gets one
    /// uses [`dates::Origin::DEMO`].
    pub fn with_origin(mut self, origin: dates::Origin) -> Session {
        self.origin = origin;
        self
    }

    /// Set the percent complete (0 to 100) per task, as `Loaded::progress`. Kept for the saved file and the
    /// export; the scene no longer draws it (a bar's fill is the engine's actuals). No date depends on it.
    pub fn with_progress(mut self, progress: Vec<u8>) -> Session {
        self.timeline.set_progress(progress);
        self
    }

    /// The calendar whose non-working days the scene shades, the data date, and the tasks whose
    /// actuals fill the bars: the scene reads progress from here and nowhere else.
    pub(crate) fn backdrop(&self) -> timeline::scene::Backdrop<'_> {
        timeline::scene::Backdrop {
            calendar: self
                .shading
                .and_then(|id| self.schedule.calendars().get(id.0 as usize)),
            data_date: self.data_date,
            origin: self.origin.axis().with_week_start(self.names.week_start),
            progress: Some(self.schedule.tasks()),
            deadlines: Some(&self.deadlines),
            start_constraints: Some(self),
            bar_colors: Some(self),
        }
    }

    /// Install a validated outline, initially with every summary expanded, and fit the view to the first screen of its rows.
    ///
    /// # Errors
    /// [`OutlineError::WrongLength`] when `outline` does not cover the schedule's tasks, and
    /// [`OutlineError::SummaryHasDependency`] when a live link ends at a summary.
    pub fn with_outline(mut self, outline: Outline) -> Result<Session, OutlineError> {
        let task_count = self.schedule.tasks().len();
        if outline.len() != task_count {
            return Err(OutlineError::WrongLength);
        }
        for (dep, dependency) in self.schedule.live_dependencies() {
            if outline.is_summary(dependency.pred) || outline.is_summary(dependency.succ) {
                return Err(OutlineError::SummaryHasDependency(dep));
            }
        }

        *self.timeline.summaries_mut() = Summaries::new(task_count);
        outline.rollup_full(&self.solution.dates, self.timeline.summaries_mut());
        let order = outline.preorder().iter().map(|task| task.0).collect();
        self.collapsed = vec![false; task_count];
        self.outline = Some(outline);
        self.timeline.set_order(order);
        crate::fit::fit_first_screen(&mut self.timeline, &self.solution);
        self.rebuild_links();
        self.sync_selection_row();
        self.stamp = self.stamp().wrapping_add(1);
        Ok(self)
    }

    /// The session that replaces this one when the page opens another file (gs#87).
    ///
    /// It takes the new schedule and builds everything that can fail (the outline) before
    /// returning, and it reads `self` without changing it, so a failed open leaves the schedule
    /// on the page as it was. The new session keeps this one's canvas size, device pixel ratio,
    /// row and axis heights, colours, month names and clock; its view is fitted to the new schedule, and it
    /// has no selection, no filter, no collapsed summary, no search results and an empty undo
    /// history. `meta` replaces this session's. `revision`
    /// starts at 0. `stamp` goes past this session's, so a table keyed on it redraws.
    ///
    /// # Errors
    /// The [`OutlineError`] of [`Session::with_outline`] when `outline` does not fit `schedule`.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per `Loaded` field it takes"
    )]
    pub fn reopen(
        &self,
        schedule: Schedule,
        solution: Solution,
        order: Vec<u32>,
        meta: Vec<TaskMeta>,
        outline: Option<Outline>,
        data_date: Option<engine::Instant>,
        origin: dates::Origin,
    ) -> Result<Session, OutlineError> {
        let old = &self.timeline();
        let (width, height, dpr) = (old.viewport.width, old.viewport.height, old.dpr);
        let metrics = old.viewport.metrics();
        let mut timeline = fitted_timeline(order, &solution, width, height, dpr);
        timeline.viewport.set_metrics(metrics);
        crate::fit::fit_first_screen(&mut timeline, &solution);
        let mut session = Session::new(schedule, solution, timeline, meta, self.clock)
            .with_data_date(data_date)
            .with_origin(origin);
        session.theme = self.theme;
        session.links.look = self.links.look;
        session.names.clone_from(&self.names);
        session.texts.clone_from(&self.texts);
        session.free_float_shown = self.free_float_shown;
        session.host.read_only = self.host.read_only;
        session.host.before_edit.clone_from(&self.host.before_edit);
        session.host.capabilities = self.host.capabilities;
        session.reports.carry_on(&self.reports);
        if let Some(outline) = outline {
            session = session.with_outline(outline)?;
        }
        session.stamp = session.stamp().wrapping_add(self.stamp()).wrapping_add(1);
        Ok(session)
    }

    /// Dependencies indexed by their current visible rows.
    pub fn links(&self) -> &LinkIndex {
        &self.links.index
    }

    /// The installed task outline, or `None` for a flat session.
    pub fn outline(&self) -> Option<&Outline> {
        self.outline.as_ref()
    }

    /// Whether `task` is a collapsed summary in the installed outline.
    pub fn is_collapsed(&self, task: TaskId) -> bool {
        self.outline.is_some()
            && self
                .collapsed
                .get(task.0 as usize)
                .copied()
                .unwrap_or(false)
    }

    /// The schedule. Read-only: only the session changes it, because the
    /// solution has to follow every change.
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// The solution of [`Session::schedule`].
    pub fn solution(&self) -> &Solution {
        &self.solution
    }

    /// The colours used to build this session's scenes.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Replace the colours used to build subsequent scenes. A theme change is display state, not a
    /// schedule edit, and therefore does not change the revision, stamp, or undo history.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Show or hide the free float column. Hidden at the start; view state, not saved and not an undo
    /// step. A change bumps [`Session::stamp`], so the table redraws.
    pub fn set_free_float_shown(&mut self, shown: bool) {
        if self.free_float_shown != shown {
            self.free_float_shown = shown;
            self.stamp = self.stamp.wrapping_add(1);
        }
    }
}
