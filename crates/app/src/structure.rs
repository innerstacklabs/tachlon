//! Insert, delete and restore (gs#50): the edits that change which tasks are live. A delete
//! tombstones a leaf in the schedule and takes it out of the outline; a restore puts it back at a
//! [`Place`]; an insert adds a new leaf there. All end in [`Session::rebuild_structure`], which builds everything that can fail
//! before it changes the session, so a refused edit leaves nothing changed.

use crate::edit::{Edit, EditError, day_minutes};
use crate::history::{Meta, Step};
use crate::outline::{Outline, OutlineError};
use crate::{Session, Summaries, meta::TaskMeta};
use engine::{CalendarId, ScheduleError, Task, TaskId};

/// The name an inserted task starts with.
const NEW_TASK: &str = "New task";

/// Where a task sits in the outline: under `parent` (`None`: a root), after sibling `after`
/// (`None`: first child).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub parent: Option<TaskId>,
    pub after: Option<TaskId>,
}

impl Place {
    /// The place `task` has in `outline`.
    pub(crate) fn of(outline: &Outline, task: TaskId) -> Place {
        let parent = outline.parent(task);
        let siblings = parent.map_or(outline.roots(), |parent| outline.children(parent));
        let at = siblings.iter().position(|&sibling| sibling == task);
        Place {
            parent,
            after: at.and_then(|at| at.checked_sub(1)).map(|at| siblings[at]),
        }
    }
}

pub(crate) type Done = Result<(Step, f64), EditError>;

impl Session {
    /// The number of live tasks: the slot count less the deleted ones.
    pub fn task_count(&self) -> usize {
        self.schedule.live_task_count()
    }

    /// Where a task inserted below `selected` goes: a sibling after its whole subtree, at its
    /// level. With none, the last top-level row. A session with no outline has no place.
    pub fn place_below(&self, selected: Option<TaskId>) -> Place {
        let Some(outline) = self.outline.as_ref() else {
            return Place {
                parent: None,
                after: None,
            };
        };
        match selected.filter(|&task| self.is_live(task)) {
            Some(task) => Place {
                parent: outline.parent(task),
                after: Some(task),
            },
            None => Place {
                parent: None,
                after: outline.roots().last().copied(),
            },
        }
    }

    /// Insert a task named `New task` below the selected subtree (see [`Session::place_below`]),
    /// recorded for undo, and select it. With nothing selected it is the last top-level row, at
    /// level 1. Refused while a row filter or lookahead window is on.
    /// Returns the new task.
    ///
    /// # Errors
    /// An [`EditError`] while a row filter or lookahead window is on, else the edit's refusal.
    pub fn insert_below_selection(&mut self) -> Result<TaskId, EditError> {
        let selected = self.selection.as_ref().map(|s| TaskId(s.task));
        let place = self.place_below(selected);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "this is the id Schedule::add_task gives the new task; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps there too"
        )]
        let id = TaskId(self.schedule.tasks().len() as u32);
        self.edit(Edit::Insert {
            place,
            name: NEW_TASK.to_string(),
        })?;
        // With nothing selected the new task is a top-level row, whatever level the last root
        // was stored at. Undo and redo keep it: they carry the level the task holds at the time.
        if !selected.is_some_and(|task| self.is_live(task)) {
            self.meta[id.0 as usize].outline_level = 1;
        }
        Ok(id)
    }

    /// Delete the selected task, recorded for undo. The selection moves to the next row's task,
    /// else the previous, else none. Returns the deleted task.
    ///
    /// # Errors
    /// [`EditError::NoSelection`] when nothing is selected, [`EditError::SeveralRows`] when a range
    /// of more than one row is, else the edit's refusal.
    pub fn delete_selected(&mut self) -> Result<TaskId, EditError> {
        if self.range_tasks().len() > 1 {
            return Err(EditError::SeveralRows);
        }
        let task = self.selection.as_ref().ok_or(EditError::NoSelection)?.task;
        self.edit(Edit::Delete { task: TaskId(task) })?;
        Ok(TaskId(task))
    }

    pub(crate) fn check_structural(&self) -> Result<(), EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        if self.outline.is_none() {
            return Err(EditError::NoOutline);
        }
        Ok(())
    }

    /// Whether `task` exists and is not deleted: the one liveness check every reader and edit
    /// shares. A deleted task reads as absent.
    pub(crate) fn is_live(&self, task: TaskId) -> bool {
        (task.0 as usize) < self.schedule.tasks().len() && !self.schedule.is_deleted(task)
    }

    /// Delete `task`. `meta`, from a history step, is put on the tombstone: an undone restore
    /// leaves the task with the parent and level it had before that restore.
    pub(crate) fn run_delete(&mut self, task: TaskId, meta: Option<Meta>) -> Done {
        self.check_structural()?;
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let outline = self.outline.as_ref().expect("checked");
        if outline.is_summary(task) {
            return Err(EditError::HasChildren {
                task,
                name: self.meta[task.0 as usize].name.clone(),
                count: outline.children(task).len(),
            });
        }
        let place = Place::of(outline, task);
        let wbs: Vec<TaskId> = outline
            .preorder()
            .iter()
            .copied()
            .filter(|&t| t != task)
            .collect();
        let was_selected = self.selection.as_ref().is_some_and(|s| s.task == task.0);
        let at = outline.preorder_position(task);
        let beside = self.screen_neighbors(task.0);
        let index = task.0 as usize;
        let held = Meta {
            parent: self.meta[index].parent,
            level: self.meta[index].outline_level,
        };
        self.schedule.delete_task(task)?;
        if let Some(meta) = meta {
            self.meta[index].parent = meta.parent;
            self.meta[index].outline_level = meta.level;
        }
        let elapsed = self.rebuild_structure(&wbs, |s| {
            let _ = s.schedule.restore_task_replay(task);
            s.meta[index].parent = held.parent;
            s.meta[index].outline_level = held.level;
        })?;
        // Only a deleted selection moves (an unselected delete keeps the selection): to the next
        // visible row's task, else the previous. The choice is made after the recompute, from the
        // rows the active filter still shows. While grouped the task rows on screen below and above
        // it, read before the rebuild, come first: users see the screen, not the outline.
        if was_selected {
            self.clear_selection();
            let row_of = |t: &u32| self.timeline().row_of_task(*t).is_some();
            let shown = beside.into_iter().flatten().find(row_of);
            if let Some(next) = shown.or_else(|| self.neighbor_row(at)) {
                self.select(next);
            }
        }
        self.refresh_selection();
        // The inverse puts back the parent and level the task held before this step.
        let inverse = Step {
            edit: Edit::Restore { task, place },
            meta: Some(held),
        };
        Ok((inverse, elapsed))
    }

    /// The preorder index `place` names: after sibling `after`'s whole subtree, else right under
    /// `parent`, else first. A place naming a task that is not live, or an `after` that is not a
    /// child of `parent`, is [`EditError::BadPlace`].
    fn position(&self, place: Place) -> Result<usize, EditError> {
        let outline = self.outline.as_ref().ok_or(EditError::NoOutline)?;
        match (place.after, place.parent) {
            (Some(after), _) if self.is_live(after) && outline.parent(after) == place.parent => {
                Ok(outline.preorder_position(after) + outline.subtree(after).len())
            }
            (None, Some(parent)) if self.is_live(parent) => {
                Ok(outline.preorder_position(parent) + 1)
            }
            (None, None) => Ok(0),
            _ => Err(EditError::BadPlace),
        }
    }

    /// The activity ID for a row inserted after the live rows `before` (preorder).
    fn next_code(&self, before: &[TaskId]) -> String {
        let code = |task: &TaskId| self.meta[task.0 as usize].code.as_deref();
        let live = self.outline.as_ref().expect("checked").preorder();
        // A row inserted ahead of every coded row follows the first one below it.
        let preceding = before
            .iter()
            .rev()
            .find_map(code)
            .or_else(|| live.iter().find_map(code));
        import::next_activity_id(preceding, live.iter().filter_map(code))
    }

    /// The calendar a task inserted at `place` gets: that of `place.after`, else the shading calendar.
    pub(crate) fn insert_calendar(&self, place: Place) -> CalendarId {
        place
            .after
            .map(|after| self.schedule.tasks()[after.0 as usize].calendar)
            .or(self.shading)
            .unwrap_or(CalendarId(0))
    }

    /// Add a leaf named `name` at `place`: a fresh id, 1 working day of the calendar of
    /// `place.after` (else the shading calendar), no constraint, and the next activity ID
    /// ([`import::next_activity_id`]). The new task is selected. A parent that is a leaf with a dependency is refused before anything changes.
    /// Any later refusal leaves the slot as a tombstone, which no reader sees.
    pub(crate) fn run_insert(&mut self, place: Place, name: &str) -> Done {
        self.check_structural()?;
        if self.filtering() {
            return Err(EditError::FilterActive);
        }
        self.refuse_grouped(EditError::InsertGrouped)?;
        let at = self.position(place)?;
        let calendar = self.insert_calendar(place);
        let duration = day_minutes(&self.schedule.calendars()[calendar.0 as usize]);
        // A sibling placed `after` keeps that sibling's level, whatever gap lies between it and
        // the parent; the first child sits one level below its parent, a first root at level 1.
        let level = match (place.after, place.parent) {
            (Some(after), _) => self.meta[after.0 as usize].outline_level,
            (None, Some(parent)) => self.meta[parent.0 as usize].outline_level.saturating_add(1),
            (None, None) => 1,
        };
        let outline = self.outline.as_ref().expect("checked");
        // A leaf with a dependency cannot become a summary (gs#47 ruling 5). Refuse before the
        // schedule gains a slot it could not give back.
        if let Some(parent) = place.parent {
            let touching = self
                .schedule
                .live_dependencies()
                .find(|(_, dependency)| dependency.pred == parent || dependency.succ == parent);
            if let Some((dep, _)) = touching.filter(|_| !outline.is_summary(parent)) {
                return Err(EditError::Outline(OutlineError::SummaryHasDependency(dep)));
            }
        }
        let mut wbs = outline.preorder().to_vec();
        let task = self.schedule.add_task(Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })?;
        let code = self.next_code(&wbs[..at]);
        wbs.insert(at, task);
        self.meta.push(TaskMeta {
            name: name.to_string(),
            parent: place.parent,
            outline_level: level,
            code: Some(code),
            activity_codes: Vec::new(),
            wbs_code: None,
            uid: None,
            floor: None,
            color: None,
            label: None,
            key: None,
        });
        self.collapsed.push(false);
        let elapsed = self.rebuild_structure(&wbs, |s| {
            let _ = s.schedule.delete_task(task);
        })?;
        self.go_to(task.0);
        self.refresh_selection();
        Ok((Step::plain(Edit::Delete { task }), elapsed))
    }

    /// Restore `task` at `place`. A person's restore (`meta` is `None`) takes its level from the
    /// parent `place` names; `meta`, from a history step, puts back exactly the parent and level
    /// the task held when the step was recorded.
    pub(crate) fn run_restore(&mut self, task: TaskId, place: Place, meta: Option<Meta>) -> Done {
        self.check_structural()?;
        if task.0 as usize >= self.schedule.tasks().len() {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        // A live task is already back; restoring it has no place to go.
        if self.is_live(task) || place.parent.is_some_and(|parent| !self.is_live(parent)) {
            return Err(EditError::BadPlace);
        }
        let at = self.position(place)?;
        let mut wbs = self.outline.as_ref().expect("checked").preorder().to_vec();
        wbs.insert(at, task);

        // A step from history (`meta` is `Some`) puts back what was recorded, duplicates of an
        // imported schedule included; only a person's restore is checked. It runs before the
        // meta changes, so a refused restore leaves the task's parent and level as they were.
        if meta.is_some() {
            self.schedule.restore_task_replay(task)?;
        } else {
            self.schedule.restore_task(task)?;
        }
        let index = task.0 as usize;
        let (old_parent, old_level) = (self.meta[index].parent, self.meta[index].outline_level);
        if let Some(meta) = meta {
            self.meta[index].parent = meta.parent;
            self.meta[index].outline_level = meta.level;
        } else {
            self.meta[index].parent = place.parent;
            self.meta[index].outline_level = place.parent.map_or(1, |parent| {
                self.meta[parent.0 as usize].outline_level.saturating_add(1)
            });
        }
        let elapsed = self.rebuild_structure(&wbs, |s| {
            let _ = s.schedule.delete_task(task);
            s.meta[index].parent = old_parent;
            s.meta[index].outline_level = old_level;
        })?;
        // Selected only while visible: a row filter hides some restored tasks, and `go_to`
        // refuses those without touching the filter or the selection.
        self.go_to(task.0);
        self.refresh_selection();
        // Undoing this restore puts back the parent and level the tombstone held before it.
        let inverse = Step {
            edit: Edit::Delete { task },
            meta: Some(Meta {
                parent: old_parent,
                level: old_level,
            }),
        };
        Ok((inverse, elapsed))
    }

    /// The task of the first visible row at or after outline position `at` (where a deleted task
    /// stood, now held by the task that followed it), else of the last visible row, else none.
    fn neighbor_row(&self, at: usize) -> Option<u32> {
        let outline = self.outline.as_ref()?;
        let order = self.timeline().order();
        if self.group.is_some() {
            // Header by header the order is not the outline's: the leaf nearest at or after the position.
            let at_of = |t: &&u32| outline.preorder_position(TaskId(**t));
            let next = order.iter().filter(|t| at_of(t) >= at).min_by_key(at_of);
            return next.or_else(|| order.iter().max_by_key(at_of)).copied();
        }
        let first = order.partition_point(|&t| outline.preorder_position(TaskId(t)) < at);
        order.get(first).or_else(|| order.last()).copied()
    }

    /// Install the outline `wbs` describes and the solution of the changed schedule. Everything
    /// that can fail happens first: on `Err`, `rollback` undoes the schedule change and nothing
    /// else has changed. Returns the milliseconds the full solve took. The caller fixes the
    /// selection, then calls `refresh_selection`.
    pub(crate) fn rebuild_structure(
        &mut self,
        wbs: &[TaskId],
        rollback: impl FnOnce(&mut Session),
    ) -> Result<f64, EditError> {
        let outline = match Outline::new(&self.meta, wbs, &self.schedule) {
            Ok(outline) => outline,
            Err(error) => {
                rollback(self);
                return Err(EditError::Outline(error));
            }
        };
        let t0 = (self.clock)();
        let solution = match self.schedule.compute() {
            Ok(solution) => solution,
            Err(error) => {
                rollback(self);
                return Err(error.into());
            }
        };
        let elapsed = (self.clock)() - t0;

        *self.timeline.summaries_mut() = Summaries::new(self.schedule.tasks().len());
        outline.rollup_full(&solution.dates, self.timeline.summaries_mut());
        self.solution = solution;
        self.outline = Some(outline);
        if !self.apply_rows() {
            self.rebuild_links();
        }
        self.found.clear();
        self.drag_info = None;
        self.changed();
        Ok(elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::Place;
    use crate::{Session, edit::Edit, meta::TaskMeta, outline::Outline};
    use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
    use timeline::Timeline;

    /// A refused restore changes nothing (gs#368): with two imported links A -> B, restoring a
    /// deleted B under C is refused as a duplicate, and B keeps the parent and level it had.
    #[test]
    fn a_refused_restore_leaves_the_meta_as_it_was() {
        let mut schedule = Schedule::new();
        let day = vec![(0, 24 * 60)];
        let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
        let calendar = schedule.add_calendar(calendar);
        let mut meta = Vec::new();
        for name in ["A", "B", "C"] {
            let task = Task {
                duration: 60,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            };
            schedule.add_task(task).expect("a task");
            let code = None;
            meta.push(TaskMeta {
                name: name.to_string(),
                parent: None,
                outline_level: 1,
                code,
                activity_codes: Vec::new(),
                wbs_code: None,
                uid: None,
                floor: None,
                color: None,
                label: None,
                key: None,
            });
        }
        let link = Dependency {
            pred: TaskId(0),
            succ: TaskId(1),
            kind: LinkType::FS,
            lag: 0,
        };
        schedule.add_dependency(link).expect("a link");
        schedule.add_dependency(link).expect("its imported twin");
        let solution = schedule.compute().expect("acyclic");
        let wbs = vec![TaskId(0), TaskId(1), TaskId(2)];
        let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
        let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
        let mut session = Session::new(schedule, solution, timeline, meta, || 0.0)
            .with_outline(outline)
            .expect("the outline fits");
        session
            .edit(Edit::Delete { task: TaskId(1) })
            .expect("deletes B");
        let before = (session.meta()[1].parent, session.meta()[1].outline_level);
        let place = Place {
            parent: Some(TaskId(2)),
            after: None,
        };
        session
            .edit(Edit::Restore {
                task: TaskId(1),
                place,
            })
            .expect_err("a person's restore of B repeats a live link");
        assert_eq!(
            (session.meta()[1].parent, session.meta()[1].outline_level),
            before
        );
    }
}
