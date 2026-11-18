//! The link records of one commit: the deletes that go with its deleted tasks and the creates that
//! go with its created ones.

use super::{Edit, Step};
use crate::Session;
use engine::{DepId, TaskId};

/// Whether a step of the commit adds `dep`: the step is a `RemoveLink`, the inverse of an add.
fn adds(steps: &[&Step], dep: DepId) -> bool {
    let adds = |s: &&Step| matches!(s.edit, Edit::RemoveLink { dep: d } if d == dep);
    steps.iter().any(adds)
}

impl Session {
    /// The link deletes that go with the `gone` tasks of one commit (decision 0017), each link
    /// once, but not a link a step of the commit adds: the links held when the commit ended, with
    /// the deleted ends of `gone` still counting as held.
    pub(crate) fn links_gone(&self, gone: &[TaskId], steps: &[&Step]) -> Vec<String> {
        let held = |t: &TaskId| !self.schedule.is_deleted(*t) || gone.contains(t);
        let mut deps: Vec<DepId> = gone
            .iter()
            .flat_map(|&t| self.schedule.task_links(t))
            .filter(|(dep, d)| !adds(steps, *dep) && held(&d.pred) && held(&d.succ))
            .map(|(dep, _)| dep)
            .collect();
        deps.sort_by_key(|dep| dep.0);
        deps.dedup();
        let delete = |dep| self.link_changes(&Edit::RestoreLink { dep });
        deps.into_iter().flat_map(delete).collect()
    }

    /// The link creates that go with the `made` tasks of one commit, each link once: the links in
    /// force after it, except those a step of the commit adds itself.
    pub(super) fn links_made(&self, made: &[TaskId], steps: &[&Step]) -> Vec<String> {
        let own = |dep| adds(steps, dep);
        let live = self.schedule.live_dependencies();
        let mine = live
            .filter(|(dep, d)| !own(*dep) && (made.contains(&d.pred) || made.contains(&d.succ)));
        mine.flat_map(|(dep, _)| self.link_changes(&Edit::RemoveLink { dep }))
            .collect()
    }

    /// The records of one step of the commit. A link the step adds and an end of which is deleted
    /// by now is listed as neither created nor deleted, and so is the undo of that step: a
    /// `RestoreLink` of a link with an end among the `made` tasks, which the host never held.
    pub(super) fn step_links(&self, edit: &Edit, made: &[TaskId]) -> Vec<String> {
        let ends = |dep: DepId| self.schedule.dependencies()[dep.0 as usize];
        let lost = |d: engine::Dependency| {
            self.schedule.is_deleted(d.pred) || self.schedule.is_deleted(d.succ)
        };
        let remade = |d: engine::Dependency| made.contains(&d.pred) || made.contains(&d.succ);
        match *edit {
            Edit::RemoveLink { dep } if lost(ends(dep)) => Vec::new(),
            Edit::RestoreLink { dep } if remade(ends(dep)) => Vec::new(),
            _ => self.link_changes(edit),
        }
    }
}
