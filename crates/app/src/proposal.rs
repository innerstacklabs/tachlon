//! A proposal (gantt#1250): changes a program asks for (a task moved, a task's length changed, a link changed) are held
//! apart from the schedule, and the chart draws a ghost where each task they move would go. The
//! schedule changes only when the person accepts them, all as one undo step or one at a time, or rejects
//! them, all or one at a time.

mod progress;

use crate::edit::{Edit, EditError, day_minutes, parse_link_text};
use crate::what_moves::{WHAT_MOVES_MAX, refuse};
use crate::{Session, history::Source};
use engine::{DepId, Dependency, Instant, Schedule, ScheduleError, Solution, TaskId};

/// The held changes and where they would put the tasks they move.
#[derive(Default, Clone)]
pub(crate) struct Proposal {
    edits: Vec<Edit>,
    /// `(task, early start, early finish)` of each task the changes move, solved when `revision` was `solved`.
    ghosts: Vec<(u32, Instant, Instant)>,
    /// The progress changes as proposed, each planned again against its task when solved or accepted.
    reports: Vec<progress::Report>,
    solved: Option<u32>,
}

impl Proposal {
    /// The tasks the held changes would move, by slot.
    pub(crate) fn tasks(&self) -> impl Iterator<Item = u32> + '_ {
        self.ghosts.iter().map(|ghost| ghost.0)
    }
}

/// What a held change is about: a task's start, a task's length, its reported progress, or the link from one task to another.
#[derive(Clone, Copy, PartialEq)]
enum Key {
    Move(TaskId),
    Duration(TaskId),
    Progress(TaskId),
    Link(TaskId, TaskId),
}

/// What `edit` changes, so that a second change to the same thing replaces the first; a link edit by slot is keyed by its pair.
fn key(schedule: &Schedule, edit: &Edit) -> Option<Key> {
    let pair = |dep: &DepId| {
        let link = schedule.dependencies().get(dep.0 as usize)?;
        Some(Key::Link(link.pred, link.succ))
    };
    match edit {
        Edit::Move { task, .. } => Some(Key::Move(*task)),
        Edit::SetDuration { task, .. } => Some(Key::Duration(*task)),
        Edit::SetProgress { task, .. } => Some(Key::Progress(*task)),
        Edit::AddLink { pred, succ, .. } => Some(Key::Link(*pred, *succ)),
        Edit::SetLink { dep, .. } | Edit::RemoveLink { dep } => pair(dep),
        _ => None,
    }
}

/// The key of the change `kind` (`"move"`, `"duration"` or `"progress"`) to `task`; `None` for any other kind.
fn task_key(kind: &str, task: u32) -> Option<Key> {
    match kind {
        "move" => Some(Key::Move(TaskId(task))),
        "duration" => Some(Key::Duration(TaskId(task))),
        "progress" => Some(Key::Progress(TaskId(task))),
        _ => None,
    }
}

impl Session {
    /// Propose that `task` starts no earlier than `start` (`YYYY-MM-DD`, or `YYYY-MM-DD HH:MM`), and
    /// answer what would move, as [`Session::what_moves`] does, with every change held so far. A
    /// second move of the task replaces the first. The schedule and its history do not change.
    /// Refused like `what_moves`, and `badDate` for text that is not a date.
    pub fn propose_move(&mut self, task: u32, start: &str) -> String {
        if let Some(refusal) = self.ask(&[task], true) {
            return refusal;
        }
        let at = (!start.trim().is_empty())
            .then(|| self.parse_start(start.trim()).ok().flatten())
            .flatten();
        match at {
            Some(at) => self.hold(Edit::Move {
                task: TaskId(task),
                snet: Some(at),
            }),
            None => refuse(
                "badDate",
                "Write the start as YYYY-MM-DD or YYYY-MM-DD HH:MM.",
            ),
        }
    }

    /// Propose that `task` takes `days` working days of its calendar (0 makes a milestone), answered
    /// and held as [`Session::propose_move`] does.
    pub fn propose_duration(&mut self, task: u32, days: u32) -> String {
        if let Some(refusal) = self.ask(&[task], true) {
            return refusal;
        }
        let minutes = i64::from(days) * day_minutes(self.schedule.calendar_of(TaskId(task)));
        self.hold(Edit::SetDuration {
            task: TaskId(task),
            minutes,
        })
    }

    /// Propose the link from `pred` to `succ` as `text` writes it (`FS`, `SS+2d`): a new link, or the
    /// new type and lag of the live one, held and answered as [`Session::propose_move`] does. A second
    /// change to the pair replaces the first. A started task is allowed; the refusals add `loop`,
    /// `alreadyLinked` (the live link is already that), `badLinkText` and `other` (a link to itself).
    pub fn propose_link(&mut self, pred: u32, succ: u32, text: &str) -> String {
        if let Some(refusal) = self.ask(&[pred, succ], false) {
            return refusal;
        }
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let live = self.live_link(pred, succ);
        let own = live.and_then(|dep| self.schedule.dependency_lag_calendar(dep));
        let day = self.pair_day_minutes(pred, succ, own);
        let (kind, lag) = match parse_link_text(text, day) {
            Ok(parsed) => parsed,
            Err(error) => return refuse("badLinkText", &self.say(&error)),
        };
        let Some(dep) = live else {
            return self.hold(Edit::AddLink {
                pred,
                succ,
                kind,
                lag,
            });
        };
        let held = self.schedule.dependencies()[dep.0 as usize];
        if (held.kind, held.lag) == (kind, lag) {
            return refuse("alreadyLinked", &self.say(&self.already_linked(pred, succ)));
        }
        self.hold(Edit::SetLink { dep, kind, lag })
    }

    /// Propose that the link from `pred` to `succ` is removed, as [`Session::propose_link`] does;
    /// `noLink` when there is none.
    pub fn propose_unlink(&mut self, pred: u32, succ: u32) -> String {
        if let Some(refusal) = self.ask(&[pred, succ], false) {
            return refusal;
        }
        match self.live_link(TaskId(pred), TaskId(succ)) {
            Some(dep) => self.hold(Edit::RemoveLink { dep }),
            None => refuse("noLink", "There is no link between those tasks."),
        }
    }

    /// The live link from `pred` to `succ`.
    fn live_link(&self, pred: TaskId, succ: TaskId) -> Option<DepId> {
        let mut links = self.schedule.live_dependencies();
        let found = links.find(|(_, link)| (link.pred, link.succ) == (pred, succ));
        found.map(|(dep, _)| dep)
    }

    /// Why a proposal about `tasks` is refused now: `busy`, `unknownTask`, `summary`, `hasActuals`
    /// (only when `started` refuses a task that has started) or, for a link to itself, `other`.
    fn ask(&self, tasks: &[u32], started: bool) -> Option<String> {
        if self.busy() {
            return Some(refuse("busy", "Finish the drag first."));
        }
        let refused = tasks.iter().find_map(|&n| self.refusal_of(n, started));
        refused.or_else(|| match *tasks {
            [a, b] if a == b => Some(refuse("other", "A task cannot be linked to itself.")),
            _ => None,
        })
    }

    /// Add `edit` to the held changes and answer what they move. At most [`WHAT_MOVES_MAX`] changes
    /// are held; a change the engine cannot solve is refused and the held ones stay as they were.
    fn hold(&mut self, edit: Edit) -> String {
        let mut edits = self.proposal.as_ref().map_or_default(|p| p.edits.clone());
        edits.retain(|held| key(&self.schedule, held) != key(&self.schedule, &edit));
        edits.push(edit);
        if edits.len() > WHAT_MOVES_MAX {
            let message = format!("Propose at most {WHAT_MOVES_MAX} changes at a time.");
            return refuse("tooMany", &message);
        }
        let solved = match self.solve_proposal(&edits) {
            Ok(solved) => solved,
            Err(error) => return self.not_solved(error),
        };
        let answer = self.solved_json(&solved);
        let ghosts = self.ghosts_of(&solved, &edits);
        let reports = self.proposal.take().map_or_default(|p| p.reports);
        self.proposal = Some(Proposal {
            edits,
            ghosts,
            reports,
            solved: Some(self.revision),
        });
        self.follow_proposal_filter();
        answer
    }

    /// A copy of the schedule with `edits` made, solved in full; the engine's refusal, or `None` for an
    /// edit a proposal does not hold.
    fn solve_proposal(&self, edits: &[Edit]) -> Result<Solution, Option<ScheduleError>> {
        let mut copy = self.schedule.clone();
        for edit in edits {
            match *edit {
                Edit::Move { task, snet } => copy.set_snet(task, snet),
                Edit::SetDuration { task, minutes } => copy.set_duration(task, minutes),
                Edit::AddLink {
                    pred,
                    succ,
                    kind,
                    lag,
                } => copy
                    .add_link(Dependency {
                        pred,
                        succ,
                        kind,
                        lag,
                    })
                    .map(drop),
                Edit::SetLink { dep, kind, lag } => copy.set_dependency(dep, kind, lag),
                Edit::RemoveLink { dep } => copy.remove_dependency(dep),
                Edit::SetProgress { .. } => progress::solve(&mut copy, edit),
                _ => return Err(None),
            }
            .map_err(Some)?;
        }
        copy.compute().map_err(Some)
    }

    /// The refusal for held changes the engine would not solve: `loop` or `alreadyLinked`, else `other`.
    fn not_solved(&self, error: Option<ScheduleError>) -> String {
        match error.map(|error| self.link_error(error)) {
            Some(e @ EditError::LinkCycle { .. }) => refuse("loop", &self.say(&e)),
            Some(e @ EditError::AlreadyLinked { .. }) => refuse("alreadyLinked", &self.say(&e)),
            _ => refuse("other", "The proposal did not solve."),
        }
    }

    /// Solve the held changes again when the schedule has changed since they were solved, so the
    /// ghosts follow an edit. It waits out a press: the frames of a drag draw the ghosts they had.
    pub fn refresh_proposal(&mut self) {
        let Some(held) = &self.proposal else { return };
        if self.busy() || held.solved == Some(self.revision) {
            return;
        }
        // A change the schedule now refuses (a link that would make a loop) draws no ghost.
        let edits: Vec<Edit> = self.live_edits(held).flatten().collect();
        let ghosts =
            (self.solve_proposal(&edits)).map_or_default(|solved| self.ghosts_of(&solved, &edits));
        if let Some(held) = &mut self.proposal {
            (held.ghosts, held.solved) = (ghosts, Some(self.revision));
        }
        self.follow_proposal_filter();
    }

    /// Accept every held change as one undo step, and answer `{"ok":true,"changes":n}`, or
    /// `{"ok":false,"reason":r,"message":m}`: `noProposal`, `busy`, or the reason the schedule
    /// refused one of them (the schedule then stays as it was and the changes stay held).
    pub fn accept_proposal(&mut self) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        let Some(held) = self.proposal.take() else {
            return refuse("noProposal", "There is no proposal to accept.");
        };
        // A task may have started or gone since it was proposed: nothing is applied, all stay held.
        let mut keys = held
            .edits
            .iter()
            .filter_map(|edit| key(&self.schedule, edit));
        if let Some(refusal) = keys.find_map(|key| self.started(key)) {
            self.proposal = Some(held);
            return refusal;
        }
        let live: Result<Vec<Edit>, EditError> = self.live_edits(&held).collect();
        let changes = held.edits.len();
        match live.and_then(|edits| self.edit_group(edits, Source::Proposal)) {
            Ok(()) => format!(r#"{{"ok":true,"changes":{changes}}}"#),
            Err(error) => {
                self.proposal = Some(held);
                refuse(error.kind(), &self.say(&error))
            }
        }
    }

    /// Drop every held change; true when there were any.
    pub fn reject_proposal(&mut self) -> bool {
        let held = self.proposal.take().is_some();
        self.follow_proposal_filter();
        held
    }

    /// Accept the one held change of `kind` (`"move"`, `"duration"` or `"progress"`) to `task` as one undo step and
    /// answer as [`Session::accept_proposal`] does (`noProposal` when that change is not held). The
    /// other changes stay held and are solved again against the schedule it made.
    pub fn accept_proposed(&mut self, kind: &str, task: u32) -> String {
        self.accept_one(task_key(kind, task))
    }

    /// Accept the one held change to the link from `pred` to `succ` (added, removed or changed), as
    /// [`Session::accept_proposed`] does.
    pub fn accept_proposed_link(&mut self, pred: u32, succ: u32) -> String {
        self.accept_one(Some(Key::Link(TaskId(pred), TaskId(succ))))
    }

    /// Drop the one held change to the link from `pred` to `succ`, as [`Session::reject_proposed`]
    /// does; true when it was held.
    pub fn reject_proposed_link(&mut self, pred: u32, succ: u32) -> bool {
        self.reject_one(Some(Key::Link(TaskId(pred), TaskId(succ))))
    }

    /// Why a held change about `key` cannot be applied: a task may have started or gone since it was
    /// proposed. A link has no such refusal here; the schedule judges it when it is applied.
    fn started(&self, key: Key) -> Option<String> {
        match key {
            Key::Move(task) | Key::Duration(task) => self.refusal(task.0),
            Key::Progress(task) => self.refusal_of(task.0, false),
            Key::Link(..) => None,
        }
    }

    fn accept_one(&mut self, key: Option<Key>) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        let Some((edit, rest)) = self.without(key) else {
            return refuse("noProposal", "That change is not held.");
        };
        // The change stays held when it is refused.
        if let Some(refusal) = key.and_then(|key| self.started(key)) {
            return refusal;
        }
        let live = self.live_edit(&rest.reports, &edit);
        match live.and_then(|edit| self.edit_group(vec![edit], Source::Proposal)) {
            Ok(()) => {
                self.keep(rest);
                r#"{"ok":true,"changes":1}"#.to_string()
            }
            Err(error) => refuse(error.kind(), &self.say(&error)),
        }
    }

    /// Drop the one held change of `kind` to `task`, record no undo step, and solve the others again
    /// without it; true when it was held.
    pub fn reject_proposed(&mut self, kind: &str, task: u32) -> bool {
        self.reject_one(task_key(kind, task))
    }

    fn reject_one(&mut self, key: Option<Key>) -> bool {
        let Some((_, rest)) = self.without(key) else {
            return false;
        };
        self.keep(rest);
        true
    }

    /// The held change about `key`, and the proposal without it.
    fn without(&self, which: Option<Key>) -> Option<(Edit, Proposal)> {
        let mut rest = self.proposal.clone()?;
        let at =
            (rest.edits.iter()).position(|e| which.is_some() && which == key(&self.schedule, e))?;
        Some((rest.edits.remove(at), rest))
    }

    /// Hold `rest` with its ghosts solved again, or nothing when no change is left.
    fn keep(&mut self, mut rest: Proposal) {
        rest.solved = None;
        self.proposal = (!rest.edits.is_empty()).then_some(rest);
        self.refresh_proposal();
        self.follow_proposal_filter();
    }

    /// Add the dashed ghosts of the held changes to `scene`, at the places they would put the
    /// tasks, above the bars, and return each ring's `(task, right edge in CSS pixels)`. Draw
    /// only: nothing here touches the schedule.
    pub fn add_proposal_ghosts(&self, scene: &mut timeline::Scene) -> Vec<(u32, f64)> {
        let Some(held) = &self.proposal else {
            return Vec::new();
        };
        let tl = self.timeline();
        timeline::scene::add_ghost_outlines(
            scene,
            &tl.viewport,
            tl.dpr,
            tl.summaries(),
            |task| tl.row_of_task(task),
            held.ghosts.iter().copied(),
            1.0,
            self.theme(),
        )
    }
}
