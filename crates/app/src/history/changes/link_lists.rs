//! The link records of a commit: the deletes and creates that say whether a link exists, and the
//! records an inverse edit of a link puts back.

use super::{Edit, Step, link_create};
use crate::{Session, json::quote};
use engine::{DepId, LagCalendar, LinkType, Minutes, TaskId};

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

    /// The link records an inverse `edit` puts back: removing a link was a create, restoring one a
    /// delete, and a set puts back a type and a lag that differ from the link's now.
    pub(crate) fn link_changes(&self, edit: &Edit) -> Vec<String> {
        let link = |dep: DepId| self.link_head(dep);
        let task = |t: TaskId| self.task_ref(t);
        let lag = |dep: DepId, m: Minutes| Self::lag_days(self.lag_day_minutes(dep), m);
        match *edit {
            Edit::RemoveLink { dep } => {
                let d = self.schedule.dependencies()[dep.0 as usize];
                let own = match self.schedule.dependency_lag_calendar(dep) {
                    None => String::new(),
                    Some(LagCalendar::Successor) => r#","lagCalendar":"successor""#.into(),
                    Some(LagCalendar::Predecessor) => r#","lagCalendar":"predecessor""#.into(),
                    Some(LagCalendar::TwentyFourHour) => r#","lagCalendar":"24h""#.into(),
                    Some(LagCalendar::Fixed(id)) => {
                        let key = self.host.keys.calendar_key(id);
                        let key = key.map_or_else(|| id.0.to_string(), quote);
                        format!(r#","lagCalendar":{{"calendar":{key}}}"#)
                    }
                };
                let ends = [task(d.pred), task(d.succ)];
                vec![link_create(
                    &link(dep),
                    ends,
                    d.kind,
                    &lag(dep, d.lag),
                    &own,
                )]
            }
            Edit::RestoreLink { dep } => {
                let gone = self.link_changes(&Edit::RemoveLink { dep });
                let delete = |c: &String| c.replacen(r#""op":"create""#, r#""op":"delete""#, 1);
                gone.iter().map(delete).collect()
            }
            Edit::SetLink {
                dep,
                kind,
                lag: old,
            } => {
                let d = self.schedule.dependencies()[dep.0 as usize];
                self.link_update(dep, (kind, old), (d.kind, d.lag))
            }
            _ => Vec::new(),
        }
    }

    /// A lag of `minutes` as days of `day` minutes, a number.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a lag in minutes is far under 2^53, so f64 holds it exactly"
    )]
    pub(crate) fn lag_days(day: Minutes, minutes: Minutes) -> String {
        format!("{:?}", minutes as f64 / day as f64)
    }

    /// The head of a link's record: its key and slot, and the day its lag is counted in.
    pub(crate) fn link_head(&self, dep: DepId) -> String {
        let key = self.host.keys.link_key(dep);
        let key = key.map_or_else(|| "null".into(), quote);
        let day = self.lag_day_minutes(dep);
        format!(
            r#""record":"link","key":{key},"link":{},"dayMinutes":{day}"#,
            dep.0
        )
    }

    /// The updates that take link `dep` from the (type, lag) pair `old` to `new`, one per value
    /// that differs.
    pub(crate) fn link_update(
        &self,
        dep: DepId,
        old: (LinkType, Minutes),
        new: (LinkType, Minutes),
    ) -> Vec<String> {
        let (head, day) = (self.link_head(dep), self.lag_day_minutes(dep));
        let (kind, lag) = (|k| format!("\"{k:?}\""), |m| Self::lag_days(day, m));
        let rows = [
            ("type", old.0 != new.0, kind(old.0), kind(new.0)),
            ("lag", old.1 != new.1, lag(old.1), lag(new.1)),
        ];
        let update = |(field, _, old, new): (&str, bool, String, String)| {
            format!(r#"{{{head},"op":"update","field":"{field}","old":{old},"new":{new}}}"#)
        };
        rows.into_iter().filter(|r| r.1).map(update).collect()
    }
}
