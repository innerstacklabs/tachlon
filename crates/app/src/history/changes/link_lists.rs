//! The link records that say whether a link exists: the deletes and creates of one commit.

use super::{Edit, Step};
use crate::Session;
use engine::{DepId, TaskId};

impl Session {
    /// The link deletes a delete of the `gone` tasks writes (decision 0017), each link once: the
    /// links held now, with the deleted ends of `gone` still counting as held.
    pub(crate) fn links_gone(&self, gone: &[TaskId]) -> Vec<String> {
        let held = |t: &TaskId| !self.schedule.is_deleted(*t) || gone.contains(t);
        let mut deps: Vec<DepId> = gone
            .iter()
            .flat_map(|&t| self.schedule.task_links(t))
            .filter(|(_, d)| held(&d.pred) && held(&d.succ))
            .map(|(dep, _)| dep)
            .collect();
        deps.sort_by_key(|dep| dep.0);
        deps.dedup();
        let delete = |dep| self.link_changes(&Edit::RestoreLink { dep });
        deps.into_iter().flat_map(delete).collect()
    }

    /// The link deletes and the link creates of one commit, for each link the commit touches: a
    /// create when the chart holds the link after the commit and did not before it, a delete when
    /// it held the link before and not after, nothing otherwise. A link is held when it is not
    /// removed and neither end is deleted. The chart now is the state after the commit. Before it, a
    /// link is not held when the commit's first step on it adds it, and a `gone` task is held while
    /// a `made` one is not.
    pub(super) fn link_lists(
        &self,
        gone: &[TaskId],
        made: &[TaskId],
        steps: &[&Step],
    ) -> (Vec<String>, Vec<String>) {
        let link = |s: &&Step| match s.edit {
            Edit::RemoveLink { dep } | Edit::RestoreLink { dep } => Some(dep),
            _ => None,
        };
        let near = gone
            .iter()
            .chain(made)
            .flat_map(|&t| self.schedule.task_links(t));
        let mut deps: Vec<DepId> = near.map(|(dep, _)| dep).collect();
        deps.extend(steps.iter().filter_map(link));
        deps.sort_by_key(|dep| dep.0);
        deps.dedup();
        let alive =
            |t: TaskId| !made.contains(&t) && (gone.contains(&t) || !self.schedule.is_deleted(t));
        let (mut deletes, mut creates) = (Vec::new(), Vec::new());
        for dep in deps {
            let d = self.schedule.dependencies()[dep.0 as usize];
            let first = steps.iter().find(|s| link(s) == Some(dep));
            let added = first.is_some_and(|s| matches!(s.edit, Edit::RemoveLink { .. }));
            let before = !added && alive(d.pred) && alive(d.succ);
            match (before, self.schedule.is_dependency_live(dep)) {
                (true, false) => deletes.extend(self.link_changes(&Edit::RestoreLink { dep })),
                (false, true) => creates.extend(self.link_changes(&Edit::RemoveLink { dep })),
                _ => {}
            }
        }
        (deletes, creates)
    }
}
