//! The pointer: a press on a bar, the drag it becomes, and the scene drawn while it
//! lasts (gs#243, split out of `lib.rs`).

mod before;
mod finished;
mod frame;
mod gesture;
mod link;
mod resize;
mod wanted;

pub(crate) use before::Before;
pub use gesture::Gesture;
pub(crate) use link::LinkState;

use crate::{Session, edit, history::Source, host::Plan, texts as t};
use engine::{Admissible, TaskId};

/// CSS pixels the pointer must travel from the press point (Euclidean)
/// before a press on a bar becomes a drag. A click, or a hand that slips a
/// pixel or two, must not edit the model.
pub const DRAG_THRESHOLD_PX: f64 = 3.0;
/// Farthest a pointer may move and still count as a click.
pub const CLICK_MAX_PX: f64 = 4.0;
/// Longest a pointer may stay down and still count as a click.
pub const CLICK_MAX_MS: f64 = 300.0;

/// What the last recompute of a drag gave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragInfo {
    pub task: u32,
    pub wanted: i64,
    pub es: i64,
    /// The task could not start where the pointer asked: its predecessors
    /// (or the working calendar) hold it later.
    pub held: bool,
    /// Time this frame spent recomputing, or zero when it recomputed nothing.
    pub recompute_ms: f64,
}

/// A press on a bar: where it landed, and the dragged task as it was before
/// the press, which a cancel or an unchanged release puts back.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Press {
    /// Pointer position of the press.
    pub(crate) at: (f64, f64),
    pub(crate) task: u32,
    /// The task's start or duration before the press, whichever the drag changes.
    pub(crate) before: Before,
    /// The task's early start before the press.
    pub(crate) es: i64,
    /// [`Session::revision`] before the press, which putting the schedule
    /// back to the press gives back.
    pub(crate) revision: u32,
}

impl Session {
    /// Pointer pressed at (x, y). A press on the link terminal, or with `alt` held on a leaf
    /// bar, starts a link drag instead of a bar press (see `press/link.rs`). A press while
    /// another is down (a second finger) is ignored: the first press ends on its own release,
    /// or on a cancel, which the page sends for a press whose release was lost.
    pub fn pointer_down(&mut self, x: f64, y: f64, alt: bool) {
        if self.gesture() != Gesture::Idle {
            return;
        }
        self.end_journal();
        self.links.reset();
        if let Some(pred) = self.link_press((x, y), alt) {
            match self.gate(&Plan::Action("linkDrag", Source::Drag, &[TaskId(pred)])) {
                Ok(()) => self.begin_link(pred, (x, y)),
                Err(error) => self.refuse_link(pred, &error),
            }
            return;
        }
        let clicked = self.timeline().hit_test(&self.solution.dates, x, y);
        self.fit_pending = false;
        self.timeline.pointer_down(&self.solution.dates, x, y);
        self.click = Some((x, y, (self.clock)(), clicked));
        // A finished task is fixed at its actuals: its press is no bar press, so the drag frames
        // find nothing to apply, and the release says why ([`Session::note_finished_drag`]).
        let bar = self.timeline().bar_drag().map(|b| b.task);
        self.finished_press = bar
            .filter(|&task| self.is_finished(TaskId(task)))
            .map(|task| (task, (x, y)));
        self.press = bar
            .filter(|_| self.finished_press.is_none())
            .map(|task| Press {
                at: (x, y),
                task,
                before: self.press_before(task),
                es: self.solution.dates[task as usize].es,
                revision: self.revision(),
            });
        self.drag_started = false;
        self.applied = None;
        // Nothing of an earlier drag carries into this press: a frame that
        // changes nothing still sets the drag info, and a stale one would
        // show an earlier drag's held outline.
        self.drag_info = None;
        self.pointer = (x, y);
    }

    /// Pointer moved to (x, y).
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        self.timeline.pointer_move(x, y);
        self.pointer = (x, y);
        self.link_move((x, y));
        if let Some(press) = self.press
            && (x - press.at.0).hypot(y - press.at.1) >= DRAG_THRESHOLD_PX
        {
            if !self.drag_started {
                // The schedule is still the press-time one: no frame has edited yet.
                match self.gate(&Plan::Drag(press)) {
                    Ok(()) => self.begin_journal(press.task),
                    Err(error) => self.refuse_drag(&error),
                }
            }
            self.drag_started = true;
        }
        self.finished_move(x, y);
    }

    /// Pointer released. A drag that moved the task is recorded as one
    /// history entry.
    pub fn pointer_up(&mut self) {
        if self.link_release() {
            return;
        }
        let click = self.click.take().filter(|&(x, y, at, _)| {
            !self.drag_started
                && (self.pointer.0 - x).hypot(self.pointer.1 - y) < CLICK_MAX_PX
                && (self.clock)() - at <= CLICK_MAX_MS
        });
        // A move and a release can land between two frames: apply the last
        // asked-for start before the drag ends, or a quick drag is lost.
        // A release before the threshold applies nothing, and neither does
        // one that asks for the task's start from before the press.
        self.apply_drag();
        self.commit_drag();
        self.note_finished_drag();
        if let Some((x, y, _, task)) = click {
            let links = &self.links.index;
            if let Some(task) = task {
                self.select(task);
            } else if let Some(arrow) = self.timeline.hit_link(links, &self.solution.dates, x, y) {
                self.select_link(engine::DepId(arrow));
            } else {
                self.clear_selection();
            }
        }
        self.end_press();
    }

    /// The gesture was cancelled (the browser's `pointercancel`): put the
    /// dragged task back as it was at the press, so nothing the drag did to
    /// the schedule stays.
    pub fn pointer_cancel(&mut self) {
        if self.link_cancel() {
            return;
        }
        if let Some(press) = self.press
            && self.drag_started
            && (press.before.held(&self.schedule, press.task) || self.restore(press).is_some())
        {
            self.drag_info = None;
        }
        self.end_press();
    }

    /// End the press, a drag included. If the dragged task has the
    /// constraint it had at the press, the schedule is the press-time
    /// schedule (a drag changes no other task), so the revision goes back to
    /// its value there. That holds however the constraint got back, and it is
    /// checked here, once, because a frame can already have set it back
    /// (leaving `revision` higher) and a later frame or the release then has
    /// nothing to set. A deferred filter refresh bumps `stamp` only when it
    /// changes the visible rows; no solution changes here.
    fn end_press(&mut self) {
        if let Some(press) = self.press
            && press.before.held(&self.schedule, press.task)
        {
            self.revision = press.revision;
        }
        self.timeline.pointer_up();
        self.end_journal();
        self.press = None;
        self.finished_press = None;
        if self.rows_stale {
            self.apply_rows();
        }
        self.drag_started = false;
        self.applied = None;
        self.click = None;
    }

    /// A bar drag is in progress: the press has moved past the threshold.
    pub fn drag_started(&self) -> bool {
        self.timeline().bar_drag().is_some() && self.drag_started
    }

    /// This frame's draw list. Before the drag starts, the pressed bar gets
    /// no outline; the timeline draws it from the press on, so a pending
    /// press is built here without it.
    pub fn scene(&self) -> timeline::Scene {
        let selected = self
            .selection
            .as_ref()
            .map(|selection| selection.row as usize);
        let range = self
            .range_rows()
            .map(|rows| (rows.first as usize, rows.last as usize));
        let mut scene = if self.timeline().bar_drag().is_some() && !self.drag_started {
            let tl = &self.timeline();
            timeline::scene::build_marked(
                &tl.viewport,
                tl.dpr,
                tl.rows(),
                self.display_dates(),
                tl.summaries(),
                timeline::scene::RowMarks {
                    hover: tl.viewport.row_at(self.pointer.1),
                    selected,
                    range,
                },
                None,
                self.backdrop(),
                &self.theme,
            )
        } else {
            self.timeline().scene(
                self.display_dates(),
                self.held(),
                selected,
                range,
                self.backdrop(),
                &self.theme,
            )
        };
        if let Some(highlight) = self.revealed_highlight() {
            timeline::scene::apply_highlight(&mut scene, highlight, &self.theme);
        }
        scene
    }

    /// Jump to the next critical task (the `c` key). Its row, if any.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row indexes the timeline order, which holds at most one entry per task, so it is below the slot count; Schedule::add_task casts that count to u32 with no check against u32::MAX, so past 2^32 slots it wraps"
    )]
    pub fn next_critical(&mut self) -> Option<u32> {
        // A jump moves the view under a still pointer, which a bar drag
        // would read as a new start: no jumping mid-drag.
        if self.busy() {
            return None;
        }
        let dates = &self.solution.dates;
        let outline = self.outline.as_ref();
        self.timeline
            .jump_to_next_critical_where(dates, |task| {
                !outline.is_some_and(|outline| outline.is_summary(TaskId(task)))
            })
            .map(|r| r as u32)
    }

    /// Whether the dragged task is held later than the pointer asked.
    pub fn held(&self) -> bool {
        let dragging = self.timeline().bar_drag().map(|b| b.task);
        self.drag_info()
            .is_some_and(|d| d.held && Some(d.task) == dragging)
    }

    /// Move the dragged task: set its start-no-earlier-than to where the
    /// pointer asks and update the solution. The engine snaps the
    /// start to working time and keeps it after its predecessors, so the bar
    /// shows the feasible result. An infeasible result keeps the last good
    /// solution and clears the constraint again. A drag that asks for the
    /// start the task had at the press leaves the schedule as it was at the
    /// press: no constraint is added, and the press ends at the revision it
    /// began with, whatever earlier frames did.
    pub fn apply_drag(&mut self) {
        let Some(b) = self.timeline().bar_drag() else {
            self.applied = None;
            return;
        };
        let Some(press) = self.press else {
            return;
        };
        if !self.drag_started {
            return;
        }
        // A stretch of the finish edge sets a duration, not a start (`press/resize.rs`).
        if let Before::Duration(_) = press.before {
            return self.resize_frame(b, press);
        }
        let task = TaskId(b.task);
        let duration = self.schedule.tasks()[b.task as usize].duration;
        let wanted = self.wanted_start(b, press);
        if self.applied == Some((b.task, wanted)) {
            return;
        }
        self.applied = Some((b.task, wanted));
        // Snapping to an admissible start is not "held": only a start later
        // than the engine's first admissible start at or after `wanted` is.
        // That rule is `Admissible`, so a milestone at a working-period end
        // counts as admissible there.
        let snapped = Admissible::new(self.schedule.calendar_of(task), duration).next(wanted);
        let current = self.schedule.tasks()[b.task as usize].snet;
        if current == Some(wanted) {
            // The schedule already has this ask, so nothing is recomputed;
            // but the diagnostic is this frame's, not the last one's: a task
            // pulled back to a start it is not held at must stop showing the
            // held outline an earlier frame of the drag set.
            let recompute_ms = 0.0;
            self.record_drag(b.task, snapped, recompute_ms);
            return;
        }
        // An ask that gives the start the task had at the press changes
        // nothing, and the rule compares the start the engine computes, not
        // the snapped ask: a task held by a predecessor keeps its start
        // however far it is dragged back toward it, and must not gain a
        // constraint the page never shows. So the ask is applied, and the
        // constraint goes back to what it was at the press if the engine's
        // start is the press start, whether or not an earlier frame of this
        // drag moved it. That recompute is the cone's, a fraction of a full
        // one. If the restore itself fails, the ask stays applied, as any
        // other frame's does.
        let (revision, stamp) = (self.revision(), self.stamp());
        let mut done = self.constrain(task, Some(wanted));
        if done.is_some()
            && self.solution.dates[b.task as usize].es == press.es
            && let Some(restored) = self.restore(press)
        {
            if press.before == Before::Snet(current) {
                // The schedule is the one this frame found: nothing changed,
                // so the revision and the stamp stay as they were. The held
                // outline is still the page's to show (the task may be held
                // by a predecessor), so the drag info below is set.
                self.revision = revision;
                self.stamp = stamp;
            } else {
                // The ask was never drawn: the restore is the one new
                // solution.
                self.stamp = stamp.wrapping_add(1);
            }
            done = Some(restored);
        }
        if let Some(recompute_ms) = done {
            self.record_drag(b.task, snapped, recompute_ms);
        }
    }

    /// Record what this frame's ask gave: the snapped ask, the early start the
    /// solution has for `task`, and whether the task is held later than the ask.
    fn record_drag(&mut self, task: u32, snapped: i64, recompute_ms: f64) {
        let es = self.solution.dates[task as usize].es;
        self.drag_info = Some(DragInfo {
            task,
            wanted: snapped,
            es,
            held: es > snapped,
            recompute_ms,
        });
    }

    /// Drag wrapper for a [`edit::Edit::Move`]. An infeasible result puts the
    /// constraint back and keeps the last good solution. Returns the
    /// recompute time in milliseconds, or `None` when the edit was refused.
    /// A task with actuals is refused before anything changes, and the drag says
    /// why in the calm note ([`Session::say_drag`]), with the task's name; the other
    /// refusals stay silent, as they were.
    fn constrain(&mut self, task: TaskId, snet: Option<i64>) -> Option<f64> {
        match self.run(edit::Edit::Move { task, snet }) {
            Ok((_, milliseconds)) => Some(milliseconds),
            Err(edit::EditError::HasActuals) => {
                let name = &self.meta[task.0 as usize].name;
                let note = self.texts.say(&t::DRAG_MOVE_STARTED, &[("name", name)]);
                self.say_drag(note);
                None
            }
            Err(_) => None,
        }
    }
}
