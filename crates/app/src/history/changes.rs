//! What a commit changed, in the vocabulary of decision 0017 (decision 0019, rule 2): the fields the
//! entry puts back, against what the session holds now. A task's fields, a link and the data date
//! carry changes; a baseline leaves none.

use super::{Entry, Source, Step};
use crate::{
    Session,
    constraint::constraint_of,
    edit::{Edit, day_minutes},
    json::quote,
    reparent::Siblings,
    structure::Place,
};
use engine::{DepId, Instant, LagCalendar, Limit, LinkType, Minutes, Pin, TaskId};

/// The fields of a task's limits, in the names of decision 0017.
pub(super) const LIMIT_FIELDS: [&str; 7] = ["snlt", "fnet", "fnlt", "mso", "mfo", "alap", "resume"];

/// A link create change: `head` names the link, `own` is the lag calendar it carries, if any.
pub(crate) fn link_create(
    head: &str,
    ends: [String; 2],
    kind: LinkType,
    lag: &str,
    own: &str,
) -> String {
    let [from, to] = ends;
    format!(
        r#"{{{head},"op":"create","fields":{{"from":{from},"to":{to},"type":"{kind:?}","lag":{lag}{own}}}}}"#
    )
}

/// The edits an entry runs, in order; a data-date step holds none.
fn edits_of(entry: &Entry) -> Vec<Edit> {
    let steps = match entry {
        Entry::Step(step) => std::slice::from_ref(step),
        Entry::Group(steps) => steps.as_slice(),
        Entry::DataDates(_) => &[],
    };
    steps.iter().map(|step| step.edit.clone()).collect()
}

impl Session {
    /// The edits the next undo (`Source::Undo`) or redo would run, in order; a data-date step holds none.
    pub(crate) fn replayed(&self, source: Source) -> Vec<Edit> {
        let entry = match source {
            Source::Undo => self.history.undo.back(),
            _ => self.history.redo.last(),
        };
        entry.map_or_else(Vec::new, edits_of)
    }

    /// The edits that undoing every entry past the first `depth` would run, newest entry first.
    pub(crate) fn discarded(&self, depth: usize) -> Vec<Edit> {
        self.history.past(depth).flat_map(edits_of).collect()
    }

    /// One JSON object per field `inverse` puts back that differs now. A field several steps touched
    /// keeps the first step's old value.
    pub(super) fn changes_of(&self, inverse: &Entry) -> Vec<String> {
        let steps: Vec<&Step> = match inverse {
            Entry::Step(step) => vec![step],
            Entry::Group(steps) => steps.iter().rev().collect(),
            Entry::DataDates(_) => Vec::new(),
        };
        let (mut seen, mut changes) = (Vec::new(), Vec::new());
        for (task, field, old) in steps.iter().flat_map(|step| self.carried(&step.edit)) {
            if seen.contains(&(task, field)) {
                continue;
            }
            seen.push((task, field));
            let new = self.value_now(task, field);
            if new != old {
                changes.push(self.task_update(task, field, &old, &new));
            }
        }
        let tasks = |gone: bool| -> Vec<TaskId> {
            let task = |s: &&Step| match s.edit {
                Edit::Restore { task, .. } if gone => Some(task),
                Edit::Delete { task } if !gone => Some(task),
                _ => None,
            };
            steps.iter().filter_map(task).collect()
        };
        changes.extend(self.links_gone(&tasks(true)));
        for step in &steps {
            changes.extend(self.link_changes(&step.edit));
            changes.extend(self.outline_changes(step));
        }
        changes.extend(self.links_made(&tasks(false), &steps));
        if let Entry::DataDates(old) = inverse {
            let (old, new) = (self.date(old.shown), self.date(self.data_date));
            if old != new {
                changes.push(format!(
                    r#"{{"record":"schedule","op":"update","field":"dataDate","old":{old},"new":{new}}}"#
                ));
            }
        }
        changes
    }

    /// The link deletes that go with the `gone` tasks of one commit (decision 0017), each link
    /// once: the links in force before it, so an end deleted in the same commit still counts.
    fn links_gone(&self, gone: &[TaskId]) -> Vec<String> {
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

    /// The link creates that go with the `made` tasks of one commit, each link once: the links in
    /// force after it, except those a step of the commit adds itself.
    fn links_made(&self, made: &[TaskId], steps: &[&Step]) -> Vec<String> {
        let own = |dep| {
            steps
                .iter()
                .any(|s| matches!(s.edit, Edit::RemoveLink { dep: d } if d == dep))
        };
        let live = self.schedule.live_dependencies();
        let mine = live
            .filter(|(dep, d)| !own(*dep) && (made.contains(&d.pred) || made.contains(&d.succ)));
        mine.flat_map(|(dep, _)| self.link_changes(&Edit::RemoveLink { dep }))
            .collect()
    }

    /// One task field's change: `old` and `new` are written as `carried` writes them.
    pub(crate) fn task_update(&self, task: TaskId, field: &str, old: &str, new: &str) -> String {
        let (key, slot) = (self.task_key(task), task.0);
        format!(
            r#"{{"record":"task","op":"update","key":{key},"task":{slot},"field":"{field}","old":{old},"new":{new}}}"#
        )
    }

    fn task_key(&self, task: TaskId) -> String {
        let key = self
            .meta
            .get(task.0 as usize)
            .and_then(|m| m.key.as_deref());
        key.map_or_else(|| "null".into(), quote)
    }

    /// The task records an inverse `step` puts back, after it ran. An inverse delete is a create
    /// (an insert, a paste, or the undo of a delete) and an inverse restore a delete; a reparent
    /// is an update of `parent` and of `after` for the task and each sibling it adopted or released.
    fn outline_changes(&self, step: &Step) -> Vec<String> {
        let head = |task: TaskId| {
            format!(
                r#""record":"task","key":{},"task":{}"#,
                self.task_key(task),
                task.0
            )
        };
        match step.edit {
            Edit::Delete { task } => {
                let fields = self.fields_of(task);
                vec![format!(
                    r#"{{{},"op":"create","fields":{fields}}}"#,
                    head(task)
                )]
            }
            Edit::Restore { task, .. } => vec![format!(r#"{{{},"op":"delete"}}"#, head(task))],
            Edit::Reparent {
                task,
                place,
                siblings,
            } => self.reparent_changes(task, place, siblings),
            _ => Vec::new(),
        }
    }

    /// `inverse` is the siblings step that puts the move back: `Release(n)` after an adopt, whose
    /// `n` siblings now are the task's last children and sat after it before; `Adopt(n)` after a
    /// release, whose `n` siblings now follow the task and were its last children before.
    fn reparent_changes(&self, task: TaskId, old: Place, inverse: Siblings) -> Vec<String> {
        let outline = self.outline.as_ref().expect("a structure edit ran");
        let kids = outline.children(task);
        let list = Place::of(outline, task)
            .parent
            .map_or(outline.roots(), |parent| outline.children(parent));
        let next = list
            .iter()
            .position(|&t| t == task)
            .map_or(list.len(), |at| at + 1);
        let (moved, mut before) = match inverse {
            Siblings::Release(n) => {
                let place = Place {
                    parent: old.parent,
                    after: Some(task),
                };
                (&kids[kids.len().saturating_sub(n)..], place)
            }
            Siblings::Adopt(n) => {
                let place = Place {
                    parent: Some(task),
                    after: kids.last().copied(),
                };
                (&list[next..(next + n).min(list.len())], place)
            }
            Siblings::Keep => (&[][..], old),
        };
        let mut rows = vec![(task, old)];
        for &t in moved {
            rows.push((t, before));
            before = Place {
                after: Some(t),
                ..before
            };
        }
        let mut changes = Vec::new();
        for (t, was) in rows {
            let now = Place::of(outline, t);
            for (field, was, now) in [
                ("parent", was.parent, now.parent),
                ("after", was.after, now.after),
            ] {
                if was != now {
                    let (was, now) = (self.place_ref(was), self.place_ref(now));
                    changes.push(self.task_update(t, field, &was, &now));
                }
            }
        }
        changes
    }

    /// The link records an inverse `edit` puts back: removing a link was a create, restoring one a
    /// delete, and a set puts back a type and a lag that differ from the link's now.
    fn link_changes(&self, edit: &Edit) -> Vec<String> {
        let link = |dep: DepId| {
            let key = self
                .host
                .keys
                .link_key(dep)
                .map_or_else(|| "null".into(), quote);
            format!(r#""record":"link","key":{key},"link":{}"#, dep.0)
        };
        let task = |t: TaskId| self.task_ref(t);
        let lag = |dep: DepId, minutes: Minutes| self.lag_days(dep, minutes);
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
            Edit::RestoreLink { dep } => vec![format!(r#"{{{},"op":"delete"}}"#, link(dep))],
            Edit::SetLink {
                dep,
                kind,
                lag: old,
            } => {
                let d = self.schedule.dependencies()[dep.0 as usize];
                let update = |field: &str, old: String, new: String| {
                    format!(
                        r#"{{{},"op":"update","field":"{field}","old":{old},"new":{new}}}"#,
                        link(dep)
                    )
                };
                let mut out = Vec::new();
                if kind != d.kind {
                    out.push(update(
                        "type",
                        format!("\"{kind:?}\""),
                        format!("\"{:?}\"", d.kind),
                    ));
                }
                if old != d.lag {
                    out.push(update("lag", lag(dep, old), lag(dep, d.lag)));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// A lag of link `dep` as days of its lag calendar, a number.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a lag in minutes is far under 2^53, so f64 holds it exactly"
    )]
    fn lag_days(&self, dep: DepId, minutes: Minutes) -> String {
        format!("{:?}", minutes as f64 / self.lag_day_minutes(dep) as f64)
    }

    pub(crate) fn carried(&self, edit: &Edit) -> Vec<(TaskId, &'static str, String)> {
        match edit {
            Edit::Move { task, snet } => vec![(*task, "snet", self.snet(*task, *snet))],
            Edit::SetDuration { task, minutes } => {
                vec![(*task, "duration", self.days(*task, Some(*minutes)))]
            }
            Edit::Rename { task, name } => vec![(*task, "name", quote(name))],
            Edit::SetProgress {
                task,
                actual_start: start,
                actual_finish: finish,
                remaining,
            } => vec![
                (*task, "actualStart", self.date(*start)),
                (*task, "actualFinish", self.date(*finish)),
                (*task, "remaining", self.days(*task, *remaining)),
            ],
            Edit::SetLimit { task, limit } => LIMIT_FIELDS
                .iter()
                .map(|field| (*task, *field, self.limit_field(limit, field)))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// One field of a task's limits, written as a host reads it.
    fn limit_field(&self, limit: &Limit, field: &str) -> String {
        let start = limit.pin.and_then(|p| match p {
            Pin::Start(at) => Some(at),
            Pin::Finish(_) => None,
        });
        let finish = limit.pin.and_then(|p| match p {
            Pin::Finish(at) => Some(at),
            Pin::Start(_) => None,
        });
        match field {
            "snlt" => self.date(limit.snlt),
            "fnet" => self.date(limit.fnet),
            "fnlt" => self.date(limit.fnlt),
            "mso" => self.date(start),
            "mfo" => self.date(finish),
            "alap" => limit.alap.to_string(),
            _ => self.date(limit.resume),
        }
    }

    /// What the session holds now for `field` of `task`, written as `carried` writes it.
    pub(crate) fn value_now(&self, task: TaskId, field: &str) -> String {
        let t = &self.schedule.tasks()[task.0 as usize];
        match field {
            "snet" => self.snet(task, t.snet),
            "duration" => self.days(task, Some(t.duration)),
            "name" => quote(&self.meta[task.0 as usize].name),
            "actualStart" => self.date(t.actual_start),
            "actualFinish" => self.date(t.actual_finish),
            "remaining" => self.days(task, t.remaining),
            _ => self.limit_field(&t.limit, field),
        }
    }

    /// A start-no-earlier-than as a host reads it: one at the floor an import set is no constraint.
    fn snet(&self, task: TaskId, snet: Option<Instant>) -> String {
        let floor = self.meta.get(task.0 as usize).and_then(|m| m.floor);
        self.date(constraint_of(snet, floor))
    }

    /// An instant as `"YYYY-MM-DDTHH:mm"`, or `null`.
    fn date(&self, at: Option<Instant>) -> String {
        let text = |at| quote(&self.date_text(at).replacen(' ', "T", 1));
        at.map_or_else(|| "null".into(), text)
    }

    /// Working minutes of `task` as days of its calendar, the default unit of decision 0017, or `null`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a duration in minutes is far under 2^53, so f64 holds it exactly"
    )]
    fn days(&self, task: TaskId, minutes: Option<Minutes>) -> String {
        let day = day_minutes(self.schedule.calendar_of(task)) as f64;
        minutes.map_or_else(|| "null".into(), |m| format!("{:?}", m as f64 / day))
    }
}
