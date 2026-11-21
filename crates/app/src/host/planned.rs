//! What the host's before-edit function is asked about (decision 0019, rules 5 and 6): the [`Plan`]
//! each gated action hands the gate, and the [`Planned`] built from it only when a function is set.

use crate::{
    Session, edit::Edit, edit::EditError, history::Source, history::link_create, json::quote,
    press::Press,
};
use engine::{LinkType, TaskId};
use std::{borrow::Cow, collections::BTreeSet, rc::Rc, slice::from_ref};

/// What a person is about to do, as the gate sees it.
pub(crate) enum Plan<'a> {
    Edit(&'a Edit),
    /// [`Session::edit_group`]: the edits in order, and what made them.
    Group(&'a [Edit], Source),
    /// Any other gated action: its kind, what made it and the tasks it touches, with no changes yet.
    Action(&'static str, Source, &'a [TaskId]),
    /// A bar drag at its threshold: the edit its first frame asks for.
    Drag(Press),
    /// An undo or redo: the step it replays, found in the history only when a function asks.
    Replay(&'static str, Source),
    /// Ending a what-if: every step since its mark, found in the history only when a function asks.
    Discard(usize),
    /// The task dialog: the texts it sends, by key.
    Dialog(TaskId, &'a dyn Fn(&str) -> Option<&'a str>),
    /// A paste: the names of the tasks it creates, trimmed.
    Paste(&'a [String]),
    /// A link drag released on a bar: the finish-to-start link it creates, `pred` to `succ`.
    Link(TaskId, TaskId),
}

/// What the before-edit function receives. `changes` are written as the outbox writes them
/// (decision 0017), old and new: for a plain edit, a group, `undo` and `redo` the task fields set;
/// for `drag` the start or duration its first frame asks for; for `setTaskFields` each field of the
/// dialog, read against the chart before the dialog applies any (except a finish after
/// a start, which is measured from the start the start edit leaves); for `insertMany`
/// a task create with its name; for `linkRelease` a link create with `from` and `to`.
/// A delete, restore, outline move, link edit and data-date edit carry the records
/// their commit writes (`values.rs`, read before the edit); an insert carries only the place and
/// name the edit names, a baseline edit the `baseline` field though its commit writes no change. A
/// plan of two or more of those (a group, a multi-step undo or redo), an undo or redo of a data-date
/// edit and `discardWhatIf` carry only `kind` and the tasks they touch. `tasks` names them, both ends
/// of a link included; an outline move names only the moved task, its siblings come with the values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// The action: `setDuration`, `rename` and the like for a plain edit, `group`, `undo`, `drag`.
    pub kind: &'static str,
    pub source: Source,
    /// The session slots of the tasks the action touches, in order of slot.
    pub tasks: Vec<u32>,
    /// One JSON object per task field that would change; `old` is what the session holds now.
    pub changes: Vec<String>,
    /// For a plan of two or more edits and for `discardWhatIf`: every record the plan would save,
    /// each read against the chart as it is, so only its `record` and `op` are right. The chart's
    /// own gate of a host's save handlers reads them; the host's function is not shown them.
    pub needs: Vec<String>,
}

/// A task field a plan changes: the task, the field, what the session holds and what it would hold.
type Field<'a> = (TaskId, &'a str, String, String);

/// The host's function, asked before each change a person makes: `Err` refuses it: [`EditError::Refused`] with its reason, or [`EditError::BeforeEditFailed`] when it failed.
pub type BeforeEdit = Rc<dyn Fn(&Planned) -> Result<(), EditError>>;

impl Session {
    /// Hand the session the host's before-edit function, or take it away with `None`.
    pub fn set_before_edit(&mut self, before: Option<BeforeEdit>) {
        self.host.before_edit = before;
    }

    pub(super) fn planned(&self, plan: &Plan) -> Planned {
        let edits: Cow<[Edit]> = match *plan {
            Plan::Edit(edit) => Cow::Borrowed(from_ref(edit)),
            Plan::Group(edits, _) => Cow::Borrowed(edits),
            Plan::Drag(press) => Cow::Owned(self.drag_edit(press).into_iter().collect()),
            Plan::Replay(_, source) => Cow::Owned(self.replayed(source)),
            Plan::Discard(depth) => Cow::Owned(self.discarded(depth)),
            Plan::Dialog(task, text) => Cow::Owned(self.dialog_edits(task, text)),
            _ => Cow::default(),
        };
        let (kind, source, tasks) = match *plan {
            Plan::Edit(edit) => (self.describe(edit).0, Source::Typed, self.touched(&edits)),
            Plan::Group(_, source) => ("group", source, self.touched(&edits)),
            Plan::Replay(kind, source) => (kind, source, self.touched(&edits)),
            Plan::Discard(_) => ("discardWhatIf", Source::Undo, self.touched(&edits)),
            Plan::Action(kind, source, tasks) => (kind, source, tasks.to_vec()),
            Plan::Drag(press) => ("drag", Source::Drag, vec![TaskId(press.task)]),
            Plan::Dialog(task, _) => ("setTaskFields", Source::Dialog, vec![task]),
            Plan::Paste(_) => ("insertMany", Source::Paste, Vec::new()),
            Plan::Link(pred, succ) => ("linkRelease", Source::Drag, vec![pred, succ]),
        };
        let slots: BTreeSet<u32> = tasks.iter().map(|task| task.0).collect();
        let mut changes = match plan {
            Plan::Discard(_) => Vec::new(),
            _ => self.planned_changes(&edits),
        };
        changes.extend(self.created(plan));
        let several = edits.len() > 1 || matches!(plan, Plan::Discard(_));
        let needs = if several {
            [self.field_changes(&edits), self.shaped_each(&edits)].concat()
        } else {
            Vec::new()
        };
        Planned {
            kind,
            source,
            tasks: slots.into_iter().collect(),
            changes,
            needs,
        }
    }

    /// The tasks `edits` touch, in order of the edits.
    fn touched(&self, edits: &[Edit]) -> Vec<TaskId> {
        edits.iter().flat_map(|e| self.describe(e).1).collect()
    }

    /// The create records a paste or a link drag would write, at the slot and number they would take.
    fn created(&self, plan: &Plan) -> Vec<String> {
        match *plan {
            Plan::Paste(names) => {
                let first = self.schedule.tasks().len();
                let create = |(n, name): (usize, &String)| {
                    let slot = first + n;
                    let name = quote(name);
                    format!(
                        r#"{{"record":"task","key":null,"task":{slot},"op":"create","fields":{{"name":{name}}}}}"#
                    )
                };
                names.iter().enumerate().map(create).collect()
            }
            Plan::Link(pred, succ) => {
                let day = self.pair_day_minutes(pred, succ, None);
                let head = format!(
                    r#""record":"link","key":null,"link":{},"dayMinutes":{day}"#,
                    self.schedule.dependencies().len()
                );
                let ends = [self.task_ref(pred), self.task_ref(succ)];
                vec![link_create(&head, ends, LinkType::FS, "0.0", "")]
            }
            _ => Vec::new(),
        }
    }

    /// An edit's name and the tasks it touches.
    fn describe(&self, edit: &Edit) -> (&'static str, Vec<TaskId>) {
        let ends = |dep: &engine::DepId| {
            let link = self.schedule.dependencies().get(dep.0 as usize);
            link.map_or_else(Vec::new, |d| vec![d.pred, d.succ])
        };
        match edit {
            Edit::Move { task, .. } => ("move", vec![*task]),
            Edit::SetDuration { task, .. } => ("setDuration", vec![*task]),
            Edit::Rename { task, .. } => ("rename", vec![*task]),
            Edit::SetProgress { task, .. } => ("setProgress", vec![*task]),
            Edit::SetLimit { task, .. } => ("setLimit", vec![*task]),
            Edit::Delete { task } => ("delete", vec![*task]),
            Edit::Restore { task, .. } => ("restore", vec![*task]),
            Edit::Reparent { task, .. } => ("reparent", vec![*task]),
            Edit::Insert { .. } => ("insert", Vec::new()),
            Edit::AddLink { pred, succ, .. } => ("addLink", vec![*pred, *succ]),
            Edit::RemoveLink { dep } => ("removeLink", ends(dep)),
            Edit::RestoreLink { dep } => ("restoreLink", ends(dep)),
            Edit::SetLink { dep, .. } => ("setLink", ends(dep)),
            Edit::SetBaseline { .. } => ("setBaseline", Vec::new()),
            Edit::ClearBaseline => ("clearBaseline", Vec::new()),
            Edit::SetDataDate { .. } => ("setDataDate", Vec::new()),
        }
    }

    /// The task-field changes of `edits` run in order against what the session holds now: the first
    /// old value and the last new one, as a commit keeps them, and none for a field that ends where
    /// it began. [`Session::carried`] reads each variant's values; [`Session::shaped_all`] adds the
    /// records of an edit that changes no task field.
    fn planned_changes(&self, edits: &[Edit]) -> Vec<String> {
        [self.field_changes(edits), self.shaped_all(edits)].concat()
    }

    /// The task-field changes of `edits`, without the records [`Session::shaped_all`] adds.
    fn field_changes(&self, edits: &[Edit]) -> Vec<String> {
        let mut fields: Vec<Field> = Vec::new();
        for edit in edits {
            if !self.describe(edit).1.iter().all(|&task| self.is_live(task)) {
                continue;
            }
            for (task, field, new) in self.carried(edit) {
                match fields.iter_mut().find(|f| (f.0, f.1) == (task, field)) {
                    Some(seen) => seen.3 = new,
                    None => fields.push((task, field, self.value_now(task, field), new)),
                }
            }
        }
        let changed = fields.iter().filter(|f| f.2 != f.3);
        let update = |(task, field, old, new): &Field| self.task_update(*task, field, old, new);
        changed.map(update).collect()
    }
}
