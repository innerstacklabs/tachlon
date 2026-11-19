//! What the before-edit function is shown for an edit that changes no task field (decision 0019,
//! rule 6): the records the commit will write, read against the chart before the edit. A link,
//! a delete and a data date come from the commit builder; a task that is not there yet carries the
//! fields the edit names.

use crate::{
    Session,
    baseline::Baseline,
    edit::Edit,
    history::{Step, link_create},
    json::quote,
    reparent::Siblings,
    structure::Place,
};
use engine::{DepId, TaskId};

/// A change of a schedule-wide value, `None` when `old` and `new` are the same.
pub(crate) fn schedule_update(field: &str, old: &str, new: &str) -> Option<String> {
    let at = format!(r#""record":"schedule","op":"update","field":"{field}""#);
    (old != new).then(|| format!(r#"{{{at},"old":{old},"new":{new}}}"#))
}

impl Session {
    /// The records of `edits` that change no task field. One such edit is read against the chart as
    /// it is; several would each read the others' work, so a plan of several carries none.
    pub(super) fn shaped_all(&self, edits: &[Edit]) -> Vec<String> {
        let mut all = edits
            .iter()
            .map(|e| self.shaped(e))
            .filter(|c| !c.is_empty());
        match (all.next(), all.next()) {
            (Some(only), None) => only,
            _ => Vec::new(),
        }
    }

    /// A task create record for `task`, which may not exist yet, with its `fields` written.
    fn task_create(&self, task: TaskId, fields: &str) -> String {
        let (key, slot, day) = (self.task_key(task), task.0, self.task_day(task));
        format!(
            r#"{{"record":"task","key":{key},"task":{slot}{day},"op":"create","fields":{fields}}}"#
        )
    }

    /// The records `edit` would write; none for an edit that names what is not there.
    fn shaped(&self, edit: &Edit) -> Vec<String> {
        let (count, deps) = (self.schedule.tasks().len(), self.schedule.dependencies());
        let live = |t: &TaskId| self.is_live(*t);
        let dep = |d: &DepId| deps.get(d.0 as usize).copied();
        let outline = self.outline.as_ref();
        match edit {
            Edit::Insert { place, name } => {
                let (parent, after) = (self.place_ref(place.parent), self.place_ref(place.after));
                let fields = format!(
                    r#"{{"parent":{parent},"after":{after},"name":{}}}"#,
                    quote(name)
                );
                vec![self.task_create(TaskId(u32::try_from(count).unwrap_or(u32::MAX)), &fields)]
            }
            Edit::Delete { task } if live(task) => outline.map_or_else(Vec::new, |outline| {
                let restore = Edit::Restore {
                    task: *task,
                    place: Place::of(outline, *task),
                };
                let mut out = self.links_gone(&[*task]);
                out.extend(self.outline_changes(&Step::plain(restore)));
                out
            }),
            Edit::Restore { task, place } if (task.0 as usize) < count && !live(task) => {
                let mut out = vec![self.task_create(*task, &self.fields_in(*task, *place))];
                let back =
                    |d: &engine::Dependency| [d.pred, d.succ].iter().all(|t| t == task || live(t));
                for (dep, _) in self.schedule.task_links(*task).filter(|(_, d)| back(d)) {
                    out.extend(self.link_changes(&Edit::RemoveLink { dep }));
                }
                out
            }
            Edit::Reparent {
                task,
                place,
                siblings,
            } if live(task) && outline.is_some() => self.moved(*task, *place, *siblings),
            Edit::AddLink {
                pred,
                succ,
                kind,
                lag,
            } if live(pred) && live(succ) => {
                let day = self.pair_day_minutes(*pred, *succ, None);
                let head = format!(
                    r#""record":"link","key":null,"link":{},"dayMinutes":{day}"#,
                    deps.len()
                );
                let ends = [self.task_ref(*pred), self.task_ref(*succ)];
                vec![link_create(
                    &head,
                    ends,
                    *kind,
                    &Self::lag_days(day, *lag),
                    "",
                )]
            }
            // The commit writes a removed link as the create that puts it back, and the reverse.
            Edit::RemoveLink { dep: d } if dep(d).is_some() => {
                self.link_changes(&Edit::RestoreLink { dep: *d })
            }
            Edit::RestoreLink { dep: d } if dep(d).is_some() => {
                self.link_changes(&Edit::RemoveLink { dep: *d })
            }
            Edit::SetLink { dep: d, kind, lag } => dep(d).map_or_else(Vec::new, |now| {
                self.link_update(*d, (now.kind, now.lag), (*kind, *lag))
            }),
            Edit::SetBaseline { .. } | Edit::ClearBaseline => self.baseline_update(edit),
            Edit::SetDataDate { date } => {
                let old = self.date(self.data_date);
                schedule_update("dataDate", &old, &self.date(*date))
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// The baseline update of a set or a clear: each side is `null` or the baseline's project
    /// finish with the data date it was taken at.
    fn baseline_update(&self, edit: &Edit) -> Vec<String> {
        let held = |finish, date| {
            let (finish, date) = (self.date(Some(finish)), self.date(date));
            format!(r#"{{"projectFinish":{finish},"dataDate":{date}}}"#)
        };
        let of = |b: &Baseline| held(b.project_finish(), b.data_date());
        let old = self.baseline.as_deref().map_or_else(|| "null".into(), of);
        let new = match edit {
            Edit::SetBaseline { baseline: Some(b) } => of(b),
            Edit::SetBaseline { baseline: None } => {
                held(self.solution.project_finish, self.data_date)
            }
            _ => "null".into(),
        };
        schedule_update("baseline", &old, &new)
            .into_iter()
            .collect()
    }

    /// The parent and after updates of a move of `task` to `new`, then of each sibling it adopts or
    /// releases, in the order the commit writes them.
    fn moved(&self, task: TaskId, new: Place, siblings: Siblings) -> Vec<String> {
        let outline = self.outline.as_ref().expect("checked");
        let old = Place::of(outline, task);
        let list = old.parent.map_or(outline.roots(), |p| outline.children(p));
        let next = list
            .iter()
            .position(|&t| t == task)
            .map_or(list.len(), |at| at + 1);
        let kids = outline.children(task);
        let (moved, mut was, mut now) = match siblings {
            Siblings::Keep => (&[][..], old, new),
            Siblings::Adopt(n) => {
                let adopted = list.get(next..next.saturating_add(n)).unwrap_or_default();
                let to = Place {
                    parent: Some(task),
                    after: kids.last().copied(),
                };
                (
                    adopted,
                    Place {
                        after: Some(task),
                        ..old
                    },
                    to,
                )
            }
            Siblings::Release(n) => {
                let from = kids.len().saturating_sub(n);
                let held = Place {
                    parent: Some(task),
                    after: from.checked_sub(1).map(|i| kids[i]),
                };
                (
                    &kids[from..],
                    held,
                    Place {
                        parent: new.parent,
                        after: Some(task),
                    },
                )
            }
        };
        let mut rows = vec![(task, old, new)];
        for &t in moved {
            rows.push((t, was, now));
            (was.after, now.after) = (Some(t), Some(t));
        }
        let mut out = Vec::new();
        for (t, was, now) in rows {
            for (field, was, now) in [
                ("parent", was.parent, now.parent),
                ("after", was.after, now.after),
            ] {
                if was != now {
                    out.push(self.task_update(
                        t,
                        field,
                        &self.place_ref(was),
                        &self.place_ref(now),
                    ));
                }
            }
        }
        out
    }
}
