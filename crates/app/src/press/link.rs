//! The link gesture and link selection (gs#49 PR 4). A press on the link terminal, or an
//! Alt-press anywhere on a leaf bar, starts a link drag from that task; dragging onto another bar
//! and releasing adds a finish-to-start link through [`Session::edit`], so it is one history step
//! and the engine's refusals reach the person unchanged. A press that never moves past the drag
//! threshold only selects the task.
//!
//! The target's verdict (summary, repeat, loop) is worked out once when the pointer reaches a new
//! target task and kept until it leaves it, so the cycle search runs once per target, not once per
//! pointer move. The session is busy for the whole drag, so no edit can change the schedule under
//! the cached verdict.

use super::DRAG_THRESHOLD_PX;
use crate::{
    Session,
    edit::{Edit, EditError},
    history::Source,
    host::Plan,
};
use engine::{DepId, LinkType, TaskId};
use timeline::scene::{LinkIndex, terminal_contains};
use view::{DEFAULT_LOOK, LinkLook};

mod view;

/// The links of the session: where they sit on rows, the selected one, and the drag in progress.
pub(crate) struct LinkState {
    pub(crate) index: LinkIndex,
    pub(crate) selected: Option<DepId>,
    drag: Option<LinkDrag>,
    pub(crate) look: LinkLook,
    /// How many times a target's loop search has run.
    checks: u32,
    /// Why the last release added nothing, until the next press.
    refusal: Option<String>,
    /// A calm note about a bar drag that moved nothing (a finished or started task), until the next press.
    note: Option<String>,
}

impl LinkState {
    pub(crate) fn new(index: LinkIndex) -> LinkState {
        LinkState {
            index,
            selected: None,
            drag: None,
            look: DEFAULT_LOOK,
            checks: 0,
            refusal: None,
            note: None,
        }
    }

    /// A press is starting: nothing of an earlier gesture carries into it.
    pub(crate) fn reset(&mut self) {
        self.drag = None;
        self.refusal = None;
        self.note = None;
    }
}

/// A link drag from `pred`.
struct LinkDrag {
    pred: u32,
    /// Where the press landed.
    at: (f64, f64),
    /// The pointer has moved [`DRAG_THRESHOLD_PX`] from `at`.
    started: bool,
    /// The task under the pointer, not `pred`.
    target: Option<Target>,
}

/// The task a drag is over and what releasing there would do.
struct Target {
    task: u32,
    /// The message a release here would be refused with, or `None` when it adds the link.
    refusal: Option<String>,
}

impl Session {
    /// A bar press, a bar drag or a link drag is in progress. Edits that move rows or the
    /// schedule, undo and redo, and the view jumps wait for it to end.
    pub(crate) fn busy(&self) -> bool {
        self.press.is_some() || self.timeline.bar_drag().is_some() || self.links.drag.is_some()
    }

    /// A link press is down, before or after the threshold.
    pub(super) fn link_down(&self) -> bool {
        self.links.drag.is_some()
    }

    /// A link drag is in progress: the press has moved past the threshold.
    pub fn linking(&self) -> bool {
        self.links.drag.as_ref().is_some_and(|drag| drag.started)
    }

    /// The selected link, if any.
    pub fn selected_link(&self) -> Option<DepId> {
        self.links.selected
    }

    /// Say why a bar drag moved nothing, as a note and not a refusal: the page shows it calmly,
    /// and the next press clears it.
    pub(crate) fn say_drag(&mut self, text: String) {
        self.links.note = Some(text);
    }

    /// The note the last bar drag left, or `None`. Cleared by the next press.
    pub fn drag_note(&self) -> Option<&str> {
        self.links.note.as_deref()
    }

    /// Why the last link drag added nothing: the engine's message for a loop, a repeat or a
    /// summary. Cleared by the next press.
    pub fn link_refusal(&self) -> Option<&str> {
        self.links.refusal.as_deref()
    }

    /// How many times a link drag has searched for a loop, once per target it reached.
    pub fn link_checks(&self) -> u32 {
        self.links.checks
    }

    /// Select link `dep`. False, and nothing changes, when it is not in force or the view shows
    /// no row for one of its ends. The selected task, if any, stays selected.
    pub fn select_link(&mut self, dep: DepId) -> bool {
        let shown = self.link_shown(dep);
        if shown {
            self.links.selected = Some(dep);
        }
        shown
    }

    /// Select the next (`forward`) or previous link of the selected task, the links that end or
    /// start at it, in the order they were added, wrapping at the ends. With no link of the task
    /// selected, forward picks the first and backward the last. Only links in force that the view
    /// shows are visited. False, and nothing changes, with no selected task, no such link, or a
    /// drag in progress.
    pub fn step_link(&mut self, forward: bool) -> bool {
        let Some(task) = self.selection().map(|selection| selection.task) else {
            return false;
        };
        if self.busy() {
            return false;
        }
        let deps = self.schedule.dependencies();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a dependency index; Schedule::add_dependency casts the count to u32 with no check against u32::MAX, so past 2^32 links it wraps, as that id does"
        )]
        let ring: Vec<DepId> = (0..deps.len() as u32)
            .map(DepId)
            .filter(|dep| {
                let d = &deps[dep.0 as usize];
                (d.pred.0 == task || d.succ.0 == task) && self.link_shown(*dep)
            })
            .collect();
        let at = self
            .links
            .selected
            .and_then(|dep| ring.iter().position(|&d| d == dep));
        let next = match (at, forward) {
            (Some(i), true) => (i + 1) % ring.len(),
            (Some(i), false) => (i + ring.len() - 1) % ring.len(),
            (None, true) => 0,
            (None, false) => ring.len().wrapping_sub(1),
        };
        match ring.get(next) {
            Some(&dep) => {
                self.links.selected = Some(dep);
                true
            }
            None => false,
        }
    }

    /// Clear the selected link.
    pub fn clear_link_selection(&mut self) {
        self.links.selected = None;
    }

    /// Remove the selected link, recorded for undo. Refused while a drag is in progress or when
    /// no link is selected. Returns the removed link.
    ///
    /// # Errors
    /// [`EditError::NoLinkSelected`] when no link is selected, else the edit's refusal.
    pub fn remove_selected_link(&mut self) -> Result<DepId, EditError> {
        let dep = self.links.selected.ok_or(EditError::NoLinkSelected)?;
        self.edit(Edit::RemoveLink { dep })?;
        Ok(dep)
    }

    /// Place the links on the current rows, and drop a selected link that is no longer in force
    /// or whose ends have no row (a collapsed summary hides them).
    pub(crate) fn rebuild_links(&mut self) {
        self.links.index = LinkIndex::of(&self.schedule, self.timeline().rows());
        if self.links.selected.is_some_and(|dep| !self.link_shown(dep)) {
            self.links.selected = None;
        }
    }

    fn link_shown(&self, dep: DepId) -> bool {
        self.schedule.is_dependency_live(dep)
            && self
                .schedule
                .dependencies()
                .get(dep.0 as usize)
                .is_some_and(|d| {
                    [d.pred, d.succ]
                        .iter()
                        .all(|task| self.timeline.row_of_task(task.0).is_some())
                })
    }

    /// The task a press at (x, y) starts a link from: the one whose terminal is under it, else,
    /// with `alt` held, the leaf bar under it.
    pub(crate) fn link_press(&self, (x, y): (f64, f64), alt: bool) -> Option<u32> {
        let tl = &self.timeline;
        let dates = &self.solution.dates;
        let on_terminal = |task: u32| {
            tl.row_of_task(task).is_some_and(|row| {
                terminal_contains(
                    &tl.viewport,
                    row,
                    &dates[task as usize],
                    self.links.look.terminal,
                    self.links.look.gap,
                    (x, y),
                )
            })
        };
        let row_task = tl.viewport.row_at(y).and_then(|row| tl.task_at_row(row));
        row_task
            .into_iter()
            .chain(self.selection.as_ref().map(|selection| selection.task))
            .find(|&task| self.leaf(task) && on_terminal(task))
            .or_else(|| {
                let bar = tl.hit_test(dates, x, y)?;
                (alt && self.leaf(bar)).then_some(bar)
            })
    }

    /// `task` is live and a leaf (a summary has a span, not a bar to link).
    fn leaf(&self, task: u32) -> bool {
        self.is_live(TaskId(task)) && self.timeline.summaries().get(task).is_none()
    }

    /// A press at `at` starts a link drag from `pred`.
    pub(crate) fn begin_link(&mut self, pred: u32, at: (f64, f64)) {
        self.timeline.pointer_up();
        self.timeline.pointer_move(at.0, at.1);
        self.press = None;
        self.click = None;
        self.drag_started = false;
        self.applied = None;
        self.drag_info = None;
        self.pointer = at;
        self.links.drag = Some(LinkDrag {
            pred,
            at,
            started: false,
            target: None,
        });
    }

    /// The pointer moved during a link press: past the threshold it is a drag, and the target
    /// follows the bar under it.
    pub(crate) fn link_move(&mut self, (x, y): (f64, f64)) {
        let Some(drag) = self.links.drag.as_mut() else {
            return;
        };
        let moved = (x - drag.at.0).hypot(y - drag.at.1);
        drag.started |= moved >= DRAG_THRESHOLD_PX;
        self.link_retarget((x, y));
    }

    /// The viewport moved under a still pointer (wheel, scroll, fit, resize): find the task under
    /// the pointer now. A task already held keeps its verdict.
    pub(crate) fn link_refresh(&mut self) {
        self.link_retarget(self.pointer);
    }

    /// Follow the task under (x, y). Past the canvas edge there is none: pointer capture keeps
    /// the events coming, but a bar that runs off the canvas is not under the pointer there.
    fn link_retarget(&mut self, (x, y): (f64, f64)) {
        let Some(drag) = self.links.drag.as_ref().filter(|drag| drag.started) else {
            return;
        };
        let pred = drag.pred;
        let held = drag.target.as_ref().map(|target| target.task);
        let under = (0.0..self.timeline.viewport.width)
            .contains(&x)
            .then(|| self.timeline.hit_test(&self.solution.dates, x, y))
            .flatten()
            .filter(|&task| task != pred);
        if under == held {
            return;
        }
        let target = under.map(|task| Target {
            task,
            refusal: self.link_verdict(pred, task),
        });
        if let Some(drag) = self.links.drag.as_mut() {
            drag.target = target;
        }
    }

    /// Why a link `pred -> succ` would be refused, in the words an edit uses, or `None`. The
    /// loop search runs only for a target that passes the cheaper checks.
    fn link_verdict(&mut self, pred: u32, succ: u32) -> Option<String> {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        if let Err(error) = self.check_link_tasks(pred, succ) {
            return Some(self.say(&error));
        }
        if self
            .schedule
            .live_dependencies()
            .any(|(_, d)| d.pred == pred && d.succ == succ)
        {
            return Some(self.say(&self.already_linked(pred, succ)));
        }
        self.links.checks += 1;
        match self.schedule.link_cycle(pred, succ) {
            Ok(None) => None,
            Ok(Some(path)) => Some(self.loop_message(&path)),
            Err(error) => Some(self.say(&self.link_error(error))),
        }
    }

    /// The pointer was released. Returns false when no link press was in progress. A press that
    /// never became a drag selects its task; a drag released over a bar adds the link, or keeps
    /// the reason it was refused in [`Session::link_refusal`]; anywhere else it adds nothing.
    pub(crate) fn link_release(&mut self) -> bool {
        // The target is the one under the pointer at the release, whatever moved the view.
        self.link_refresh();
        let Some(drag) = self.links.drag.take() else {
            return false;
        };
        self.timeline.pointer_up();
        if !drag.started {
            self.select(drag.pred);
            return true;
        }
        let Some(target) = drag.target else {
            return true;
        };
        let ends = [TaskId(drag.pred), TaskId(target.task)];
        let add = Edit::AddLink {
            pred: ends[0],
            succ: ends[1],
            kind: LinkType::FS,
            lag: 0,
        };
        let result = match target.refusal {
            Some(message) => Err(message),
            None => self
                .gate(&Plan::Link(ends[0], ends[1]))
                .and_then(|()| self.edit_from(add, Source::Drag))
                .map_err(|error| self.say(&error)),
        };
        self.links.refusal = result.err();
        true
    }

    /// The link drag was cancelled: nothing was added.
    pub(crate) fn link_cancel(&mut self) -> bool {
        let was = self.links.drag.take().is_some();
        if was {
            self.timeline.pointer_up();
        }
        was
    }
}
