//! Checked edits of a link: add, remove, restore, and the loop a new link
//! would close. `add_dependency` stays the unchecked door for importers.

use crate::calendar::Minutes;
use crate::model::range::check_lag;
use crate::model::{DepId, Dependency, LinkType, Schedule, ScheduleError, TaskId};

impl Schedule {
    /// The dependencies in force: not removed, neither end deleted. The
    /// [`DepId`] is the slot's own.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "i is a dependency slot below the dependency count, which DepId holds in u32 \
                  (unchecked at Schedule::add_dependency)"
    )]
    pub fn live_dependencies(&self) -> impl Iterator<Item = (DepId, &Dependency)> + '_ {
        self.deps
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.live_at(i))
            .map(|(i, d)| (DepId(i as u32), d))
    }

    /// The links to or from `task` that were not removed on their own, whether an end is deleted
    /// or not: the ones a delete of `task` takes out of force and a restore can bring back.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "i is a dependency slot below the dependency count, which DepId holds in u32"
    )]
    pub fn task_links(&self, task: TaskId) -> impl Iterator<Item = (DepId, &Dependency)> + '_ {
        let deps = self.deps.iter().enumerate();
        let mine =
            deps.filter(move |(i, d)| !self.removed[*i] && (d.pred == task || d.succ == task));
        mine.map(|(i, d)| (DepId(i as u32), d))
    }

    /// The loop a link `pred -> succ` would close, or `None`. The path runs
    /// `pred, succ, ..` and back: each task is a predecessor of the next, the
    /// last of the first, as in [`ScheduleError::Cycle`]. A search on the
    /// cached graph (built if absent); nothing changes. Refuses a deleted or
    /// unknown end and `pred == succ`; an existing loop in the schedule is
    /// reported as [`ScheduleError::Cycle`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted end,
    /// [`ScheduleError::SelfDependency`] if `pred == succ`, and [`ScheduleError::Cycle`] if the
    /// schedule already holds a loop.
    #[expect(
        clippy::missing_panics_doc,
        reason = "the expect reads the graph built just above"
    )]
    pub fn link_cycle(
        &mut self,
        pred: TaskId,
        succ: TaskId,
    ) -> Result<Option<Vec<TaskId>>, ScheduleError> {
        self.check_ends(pred, succ)?;
        let n = self.tasks.len();
        self.graph()?;
        let g = self.graph.as_ref().expect("just built");
        // Walk successors from `succ`; reaching `pred` closes the loop.
        let mut from = vec![u32::MAX; n];
        from[succ.0 as usize] = succ.0;
        let mut queue = vec![succ.0];
        let mut head = 0;
        while head < queue.len() {
            let t = queue[head] as usize;
            head += 1;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "t came from a u32 task id in the queue, so it fits u32"
            )]
            for &d in &g.succ_deps[g.succ_start[t] as usize..g.succ_start[t + 1] as usize] {
                let s = self.deps[d as usize].succ.0;
                if from[s as usize] != u32::MAX {
                    continue;
                }
                from[s as usize] = t as u32;
                if s == pred.0 {
                    let mut path = vec![pred];
                    let mut cur = from[s as usize];
                    while cur != succ.0 {
                        path.push(TaskId(cur));
                        cur = from[cur as usize];
                    }
                    path.push(succ);
                    // `path` runs pred, then back toward succ; flip the tail.
                    path[1..].reverse();
                    return Ok(Some(path));
                }
                queue.push(s);
            }
        }
        Ok(None)
    }

    /// Add a link for a person: both ends live, not a task to itself, no live
    /// link already from `pred` to `succ` (any type; one the other way is
    /// fine), no loop. On `Err` nothing changes and the cached graph stays.
    /// Invalidates the cached topological order on success.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown or deleted end,
    /// [`ScheduleError::SelfDependency`] if `dep.pred == dep.succ`,
    /// [`ScheduleError::DuplicateDependency`] if a live link already runs from `dep.pred` to
    /// `dep.succ`, and [`ScheduleError::Cycle`] if the link would close a loop.
    pub fn add_link(&mut self, dep: Dependency) -> Result<DepId, ScheduleError> {
        self.check_new_link(dep.pred, dep.succ)?;
        self.add_dependency(dep)
    }

    /// Change a live link's type and lag, checked like [`Schedule::add_link`]: the id must name a
    /// link in force (a removed or unknown one is [`ScheduleError::UnknownDependency`]), no other
    /// live link may already run `pred -> succ` with the new type, unless the type is the one the
    /// link has (a lag change makes no new twin, so an imported twin does not stop it;
    /// [`ScheduleError::DuplicateDependency`] names the other), and the pair may close no loop
    /// ([`ScheduleError::Cycle`]; a change of type or lag adds no edge, so that is a loop the
    /// schedule already held). The ends, the lag calendar and the slot stay. On `Err` nothing
    /// changes. Keeps the cached topological order: the edges are the same. Solve again after a
    /// change: a stored [`crate::Solution`] still holds the old dates.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] if `dep` is not a link in force,
    /// [`ScheduleError::DuplicateDependency`] if another live link already runs `pred -> succ` with
    /// the new type, [`ScheduleError::Cycle`] if the schedule already holds a loop through the
    /// link, and [`ScheduleError::OutOfRange`] for a lag beyond [`MAX_LAG`](crate::MAX_LAG).
    pub fn set_dependency(
        &mut self,
        dep: DepId,
        kind: LinkType,
        lag: Minutes,
    ) -> Result<(), ScheduleError> {
        if !self.is_dependency_live(dep) {
            return Err(ScheduleError::UnknownDependency(dep));
        }
        let Dependency { pred, succ, .. } = self.deps[dep.0 as usize];
        self.check_ends(pred, succ)?;
        if kind != self.deps[dep.0 as usize].kind
            && let Some((twin, _)) = self
                .live_dependencies()
                .find(|&(id, d)| id != dep && d.pred == pred && d.succ == succ && d.kind == kind)
        {
            return Err(ScheduleError::DuplicateDependency(twin));
        }
        if let Some(path) = self.link_cycle(pred, succ)? {
            return Err(ScheduleError::Cycle(path));
        }
        self.set_dependency_replay(dep, kind, lag)
    }

    /// Put a live link's type and lag back exactly as history recorded them: for undo, redo and
    /// rollback only. It skips the duplicate and loop checks of [`Schedule::set_dependency`],
    /// because the state it restores was accepted before (an import can hold two identical links,
    /// and undoing a type change must bring the first one back beside its twin). An unknown or
    /// removed id is refused with [`ScheduleError::UnknownDependency`]; on `Err` nothing changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] for an unknown or removed id and
    /// [`ScheduleError::OutOfRange`] for a lag beyond [`MAX_LAG`](crate::MAX_LAG).
    pub fn set_dependency_replay(
        &mut self,
        dep: DepId,
        kind: LinkType,
        lag: Minutes,
    ) -> Result<(), ScheduleError> {
        if !self.is_dependency_live(dep) {
            return Err(ScheduleError::UnknownDependency(dep));
        }
        check_lag(dep, lag)?;
        let d = &mut self.deps[dep.0 as usize];
        d.kind = kind;
        d.lag = lag;
        Ok(())
    }

    /// Take a link out of force. It keeps its [`DepId`] and its slot, so
    /// [`Schedule::restore_dependency`] brings it back as it was. An unknown or
    /// already removed id is refused with [`ScheduleError::UnknownDependency`]
    /// and nothing changes. Invalidates the cached topological order.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] for an unknown or already removed id.
    pub fn remove_dependency(&mut self, dep: DepId) -> Result<(), ScheduleError> {
        match self.removed.get_mut(dep.0 as usize) {
            Some(r) if !*r => *r = true,
            _ => return Err(ScheduleError::UnknownDependency(dep)),
        }
        self.graph = None;
        Ok(())
    }

    /// Bring a removed link back, checked like [`Schedule::add_link`] (the
    /// pair may have a live link or a path the other way by now). A link that
    /// is not removed is left as it is; an unknown id is refused with
    /// [`ScheduleError::UnknownDependency`]. A link with a deleted end is
    /// refused with [`ScheduleError::UnknownTask`] until that task is
    /// restored; `restore_task` does not bring back a link that was removed
    /// on its own, so call this again after the task is back.
    /// On `Err` nothing changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] for an unknown id, [`ScheduleError::UnknownTask`]
    /// for a link with a deleted end, and [`ScheduleError::DuplicateDependency`] or
    /// [`ScheduleError::Cycle`] if the link would repeat a live one or close a loop.
    pub fn restore_dependency(&mut self, dep: DepId) -> Result<(), ScheduleError> {
        self.restore_link(dep, true)
    }

    /// Bring a removed link back exactly as history recorded it: for undo and
    /// redo history only. It skips the duplicate and loop checks of
    /// [`Schedule::restore_dependency`], because a link that was accepted
    /// before (an import can hold two links from A to B) must come back even
    /// when its twin is live. It keeps the structural checks: an unknown id is
    /// refused with [`ScheduleError::UnknownDependency`], a link with a
    /// deleted end with [`ScheduleError::UnknownTask`], and a link that is
    /// not removed is left as it is. On `Err` nothing changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownDependency`] for an unknown id and
    /// [`ScheduleError::UnknownTask`] for a link with a deleted end.
    pub fn restore_dependency_replay(&mut self, dep: DepId) -> Result<(), ScheduleError> {
        self.restore_link(dep, false)
    }

    fn restore_link(&mut self, dep: DepId, checked: bool) -> Result<(), ScheduleError> {
        let i = dep.0 as usize;
        match self.removed.get(i) {
            None => return Err(ScheduleError::UnknownDependency(dep)),
            Some(false) => return Ok(()),
            Some(true) => {}
        }
        let d = self.deps[i];
        if checked {
            self.check_new_link(d.pred, d.succ)?;
        } else {
            self.check_ends(d.pred, d.succ)?;
        }
        self.removed[i] = false;
        self.graph = None;
        Ok(())
    }

    /// Undo/redo history only. Put a deleted task back exactly as it was, with
    /// every link it had, and without the duplicate and loop checks of
    /// [`Schedule::restore_task`]: history records a state that was already
    /// there, and an imported schedule may hold repeated links the person
    /// never added. A person's restore calls `restore_task`. The structural
    /// checks stay: an unknown id is refused with [`ScheduleError::UnknownTask`]
    /// and a live task is left as it is. Invalidates the cached topological
    /// order when it changes anything.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::UnknownTask`] for an unknown id.
    pub fn restore_task_replay(&mut self, task: TaskId) -> Result<(), ScheduleError> {
        let t = self
            .tasks
            .get_mut(task.0 as usize)
            .ok_or(ScheduleError::UnknownTask(task))?;
        if t.deleted {
            t.deleted = false;
            self.graph = None;
        }
        Ok(())
    }

    /// Undelete `task`, bringing its links back one at a time, each checked
    /// like [`Schedule::restore_dependency`]. On `Err` the task is deleted
    /// again, every flag and the cached graph are as they were.
    pub(crate) fn restore_checked(&mut self, task: TaskId) -> Result<(), ScheduleError> {
        let i = task.0 as usize;
        // Links this restore puts back in force: not removed, other end live.
        let back: Vec<usize> = (0..self.deps.len())
            .filter(|&k| {
                let d = self.deps[k];
                let other = if d.pred == task { d.succ } else { d.pred };
                (d.pred == task || d.succ == task)
                    && !self.removed[k]
                    && !self.tasks[other.0 as usize].deleted
            })
            .collect();
        let saved = self.graph.take();
        for &k in &back {
            self.removed[k] = true;
        }
        self.tasks[i].deleted = false;
        self.graph = None;
        let mut result = Ok(());
        for (n, &k) in back.iter().enumerate() {
            let d = self.deps[k];
            if let Err(e) = self.check_new_link(d.pred, d.succ) {
                for &j in &back[n..] {
                    self.removed[j] = true;
                }
                result = Err(e);
                break;
            }
            self.removed[k] = false;
            self.graph = None;
        }
        if result.is_err() {
            for &j in &back {
                self.removed[j] = false;
            }
            self.tasks[i].deleted = true;
            self.graph = saved;
        }
        result
    }

    fn check_ends(&self, pred: TaskId, succ: TaskId) -> Result<(), ScheduleError> {
        for t in [pred, succ] {
            if self.tasks.get(t.0 as usize).is_none_or(|x| x.deleted) {
                return Err(ScheduleError::UnknownTask(t));
            }
        }
        if pred == succ {
            return Err(ScheduleError::SelfDependency(pred));
        }
        Ok(())
    }

    /// Refuse a live link `pred -> succ` that would repeat one or close a loop.
    fn check_new_link(&mut self, pred: TaskId, succ: TaskId) -> Result<(), ScheduleError> {
        self.check_ends(pred, succ)?;
        self.graph()?;
        let g = self.graph.as_ref().expect("just built");
        let p = pred.0 as usize;
        let same = g.succ_deps[g.succ_start[p] as usize..g.succ_start[p + 1] as usize]
            .iter()
            .find(|&&d| self.deps[d as usize].succ == succ);
        if let Some(&d) = same {
            return Err(ScheduleError::DuplicateDependency(DepId(d)));
        }
        match self.link_cycle(pred, succ)? {
            Some(path) => Err(ScheduleError::Cycle(path)),
            None => Ok(()),
        }
    }
}
