//! Link edits (gs#49): add, remove, restore and set a link. Each one refuses a busy session and a
//! summary, lets the schedule judge duplicates and loops, then runs a full solve: a link changes
//! its predecessor's late dates and float even when no early date moves, which the forward cone
//! does not cover. A failed solve rolls the link back, so a refusal changes nothing.

use super::{Edit, EditError};
use crate::Session;
use crate::texts::{LINK_CYCLE, LINK_CYCLE_ELIDED, Texts};
use engine::{DepId, Dependency, LinkType, Minutes, Moved, ScheduleError, TaskDates, TaskId};

/// Most tasks of a loop the message names before it elides the middle.
const SHOWN: usize = 5;

type Done = Result<(Edit, f64), EditError>;

/// Put in `out` each task whose early dates differ between `old` and `new`, with the old ones,
/// in task order: what the engine's cone lists for a duration edit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
)]
fn moved_between(old: &[TaskDates], new: &[TaskDates], out: &mut Vec<Moved>) {
    out.clear();
    out.extend(
        old.iter()
            .zip(new)
            .enumerate()
            .filter(|(_, (was, now))| was.es != now.es || was.ef != now.ef)
            .map(|(index, (was, _))| Moved {
                task: TaskId(index as u32),
                was_es: was.es,
                was_ef: was.ef,
            }),
    );
}

impl Session {
    pub(crate) fn run_add_link(
        &mut self,
        pred: TaskId,
        succ: TaskId,
        kind: LinkType,
        lag: Minutes,
    ) -> Done {
        self.check_link_ends(pred, succ)?;
        let dep = self
            .schedule
            .add_link(Dependency {
                pred,
                succ,
                kind,
                lag,
            })
            .map_err(|error| self.link_error(error))?;
        // The slot stays as a removed tombstone, as an insert's does.
        let elapsed = self.solve_links(|s| {
            let _ = s.schedule.remove_dependency(dep);
        })?;
        self.links.selected = Some(dep);
        Ok((Edit::RemoveLink { dep }, elapsed))
    }

    /// Change a live link's type and lag. The inverse is the same edit with the old pair, so undo
    /// and redo put back the type and lag exactly; the link stays selected. `replay` is history
    /// putting back what it recorded: the schedule skips its duplicate check (an import can hold
    /// two identical links), the session keeps the rest. A failed solve is rolled back the same way.
    pub(crate) fn run_set_link(
        &mut self,
        dep: DepId,
        kind: LinkType,
        lag: Minutes,
        replay: bool,
    ) -> Done {
        self.check_link_busy()?;
        if !self.schedule.is_dependency_live(dep) {
            return Err(EditError::UnknownLink(dep));
        }
        let before = self.schedule.dependencies()[dep.0 as usize];
        self.check_link_tasks(before.pred, before.succ)?;
        if replay {
            self.schedule.set_dependency_replay(dep, kind, lag)
        } else {
            self.schedule.set_dependency(dep, kind, lag)
        }
        .map_err(|error| self.link_error(error))?;
        let elapsed = self.solve_links(|s| {
            let _ = s
                .schedule
                .set_dependency_replay(dep, before.kind, before.lag);
        })?;
        Ok((
            Edit::SetLink {
                dep,
                kind: before.kind,
                lag: before.lag,
            },
            elapsed,
        ))
    }

    pub(crate) fn run_remove_link(&mut self, dep: DepId) -> Done {
        self.check_link_busy()?;
        if !self.schedule.is_dependency_live(dep) {
            return Err(EditError::UnknownLink(dep));
        }
        self.schedule
            .remove_dependency(dep)
            .map_err(|error| self.link_error(error))?;
        let elapsed = self.solve_links(|s| {
            let _ = s.schedule.restore_dependency_replay(dep);
        })?;
        self.links.selected = None;
        Ok((Edit::RestoreLink { dep }, elapsed))
    }

    /// `replay` is undo and redo putting back what history recorded: the schedule skips its duplicate
    /// and loop checks (an import can hold two links on a pair), the session keeps the rest.
    pub(crate) fn run_restore_link(&mut self, dep: DepId, replay: bool) -> Done {
        self.check_link_busy()?;
        let link = *self
            .schedule
            .dependencies()
            .get(dep.0 as usize)
            .ok_or(EditError::UnknownLink(dep))?;
        // A live link is not removed: there is nothing to restore, and undoing it would delete it.
        if self.schedule.is_dependency_live(dep) {
            return Err(self.already_linked(link.pred, link.succ));
        }
        self.check_link_ends(link.pred, link.succ)?;
        if replay {
            self.schedule.restore_dependency_replay(dep)
        } else {
            self.schedule.restore_dependency(dep)
        }
        .map_err(|error| self.link_error(error))?;
        let elapsed = self.solve_links(|s| {
            let _ = s.schedule.remove_dependency(dep);
        })?;
        self.links.selected = Some(dep);
        Ok((Edit::RemoveLink { dep }, elapsed))
    }

    fn check_link_busy(&self) -> Result<(), EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        Ok(())
    }

    /// Busy, then each end: live and not a summary (decision 0002, Decision 4).
    fn check_link_ends(&self, pred: TaskId, succ: TaskId) -> Result<(), EditError> {
        self.check_link_busy()?;
        self.check_link_tasks(pred, succ)
    }

    /// Each end is live and not a summary. A link drag asks this of its target before the release.
    pub(crate) fn check_link_tasks(&self, pred: TaskId, succ: TaskId) -> Result<(), EditError> {
        for task in [pred, succ] {
            if !self.is_live(task) {
                return Err(ScheduleError::UnknownTask(task).into());
            }
            if self.outline.as_ref().is_some_and(|o| o.is_summary(task)) {
                return Err(EditError::LinkToSummary {
                    name: self.link_name(task),
                });
            }
        }
        Ok(())
    }

    /// Full solve after a link change, then everything that follows a changed solution, the move
    /// report included: the old early dates are kept, the tasks whose dates differ become the
    /// moved list, and the report is the one a duration edit makes (no task is the edited one, so
    /// all of them count). On a solve error `rollback` undoes the schedule change and nothing else
    /// has changed. Returns the milliseconds the solve took.
    fn solve_links(&mut self, rollback: impl FnOnce(&mut Session)) -> Result<f64, EditError> {
        self.solve_full(None, rollback)
    }

    /// [`Session::solve_links`] for an edit of `edited`: that task is reported apart and left out
    /// of the count, as a duration edit's is.
    pub(crate) fn solve_full(
        &mut self,
        edited: Option<TaskId>,
        rollback: impl FnOnce(&mut Session),
    ) -> Result<f64, EditError> {
        let old_finish = self.solution.project_finish;
        let held = self.reports.held;
        self.remember_critical(edited);
        let t0 = (self.clock)();
        let solution = match self.schedule.compute() {
            Ok(solution) => solution,
            Err(error) => {
                self.forget_critical();
                rollback(self);
                return Err(error.into());
            }
        };
        let elapsed = (self.clock)() - t0;
        let old = std::mem::replace(&mut self.solution, solution);
        self.rollup_full_summaries();
        self.rebuild_links();
        self.follow_filter();
        moved_between(&old.dates, &self.solution.dates, &mut self.moved);
        self.changed();
        match edited {
            Some(task) => self.note_moved(task, old_finish, held),
            None => self.note_links_moved(old_finish, held),
        }
        Ok(elapsed)
    }

    /// The task as a refusal names it: its own name, after its activity ID when it has one.
    fn link_name(&self, task: TaskId) -> String {
        let meta = &self.meta[task.0 as usize];
        match &meta.code {
            Some(code) => format!("{code} {}", meta.name),
            None => meta.name.clone(),
        }
    }

    pub(crate) fn already_linked(&self, pred: TaskId, succ: TaskId) -> EditError {
        EditError::AlreadyLinked {
            pred,
            succ,
            pred_name: self.link_name(pred),
            succ_name: self.link_name(succ),
        }
    }

    /// The schedule's refusal as the person reads it.
    pub(crate) fn link_error(&self, error: ScheduleError) -> EditError {
        match error {
            ScheduleError::Cycle(path) if path.len() >= 2 => EditError::LinkCycle {
                names: self.loop_names(&path),
                path,
            },
            ScheduleError::DuplicateDependency(dep) => {
                let link = self.schedule.dependencies()[dep.0 as usize];
                self.already_linked(link.pred, link.succ)
            }
            ScheduleError::UnknownDependency(dep) => EditError::UnknownLink(dep),
            error => error.into(),
        }
    }

    /// The loop `path` (`A, B, ..` as [`engine::Schedule::link_cycle`] returns it) by name,
    /// `B, .., A`.
    pub(crate) fn loop_names(&self, path: &[TaskId]) -> Vec<String> {
        path[1..]
            .iter()
            .chain(&path[..1])
            .map(|&task| self.link_name(task))
            .collect()
    }

    /// [`loop_text`] of `path`, in this chart's words.
    pub(crate) fn loop_message(&self, path: &[TaskId]) -> String {
        loop_text(&self.texts, &self.loop_names(path))
    }
}

/// `"A" to "B" would make a loop: B → C → A → B.` `names` run `B, C, A` as
/// [`Session::loop_names`] gives them. Past [`SHOWN`] tasks the middle is elided:
/// `B → C → D → … (12 tasks) → A → B`.
pub(super) fn loop_text(texts: &Texts, names: &[String]) -> String {
    let chain = if names.len() > SHOWN {
        let holes = [
            ("first", &*names[0]),
            ("second", &*names[1]),
            ("third", &*names[2]),
            ("count", &*names.len().to_string()),
            ("last", &*names[names.len() - 1]),
        ];
        texts.say(&LINK_CYCLE_ELIDED, &holes)
    } else {
        format!("{} → {}", names.join(" → "), names[0])
    };
    let (succ, pred) = (&names[0], &names[names.len() - 1]);
    texts.say(
        &LINK_CYCLE,
        &[("pred", pred), ("succ", succ), ("chain", &chain)],
    )
}
