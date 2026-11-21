//! Undo and redo history for person-initiated edits and completed drags.

mod changes;
mod commit;
mod forget;
mod record;
mod rollback;

use crate::whatif::Mark;
use crate::{
    Session,
    edit::{Edit, EditError},
    host::Plan,
};
pub(crate) use changes::link_create;
pub use commit::Source;
use engine::{Instant, ScheduleError, TaskId};
use std::collections::VecDeque;

/// Most steps the history keeps; the oldest is dropped past it.
pub const HISTORY_CAP: usize = 200;

/// The parent and outline level a delete, restore or reparent puts on a task as part of its step. They live
/// in history's inverse records only: no public [`Edit`] carries them, so a person's restore
/// always derives its level from the parent it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Meta {
    pub(crate) parent: Option<TaskId>,
    pub(crate) level: u8,
}

/// One history entry: the edit to apply, and for a delete, restore or reparent the exact meta to put back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) edit: Edit,
    pub(crate) meta: Option<Meta>,
}

impl Step {
    pub(crate) fn plain(edit: Edit) -> Self {
        Self { edit, meta: None }
    }
}

/// The two data dates a session holds: the schedule's floor and the date the scene marks. They
/// differ for a file opened before the loader sets the floor (gs#736), so a data-date step keeps
/// both and undo puts back exactly what was there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DataDates {
    pub(crate) floor: Option<Instant>,
    pub(crate) shown: Option<Instant>,
}

impl DataDates {
    /// Both at `date`: what a person's edit sets.
    pub(crate) fn both(date: Option<Instant>) -> Self {
        Self {
            floor: date,
            shown: date,
        }
    }
}

/// What a history stack holds: an edit to apply, or the data dates to put back.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Step(Step),
    DataDates(DataDates),
    /// Several steps one action made, run in this order as one undo step.
    Group(Vec<Step>),
    /// An action the host took outside the chart; its token is all the chart keeps.
    Host(String),
}

/// The task a history entry changes, for an edit of one row; none for a link, baseline or data-date step.
fn task_of(entry: &Entry) -> Option<TaskId> {
    match entry {
        Entry::Step(Step {
            edit:
                Edit::Move { task, .. }
                | Edit::SetDuration { task, .. }
                | Edit::Rename { task, .. }
                | Edit::SetProgress { task, .. }
                | Edit::Delete { task }
                | Edit::Restore { task, .. }
                | Edit::Reparent { task, .. },
            ..
        }) => Some(*task),
        _ => None,
    }
}

/// Two stacks of inverse entries: what undo applies, and what redo applies.
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    cap: usize,
    /// The what-if mark, if one is on.
    pub(crate) whatif: Option<Mark>,
}

impl History {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            cap,
            whatif: None,
        }
    }

    /// The undo entries past the first `depth`, newest first.
    fn past(&self, depth: usize) -> impl Iterator<Item = &Entry> {
        self.undo.iter().skip(depth).rev()
    }

    /// The latest undo entry becomes `inverse` on the redo stack.
    fn move_to_redo(&mut self, inverse: Entry) {
        self.undo.pop_back();
        self.redo.push(inverse);
    }

    /// The latest redo entry becomes `inverse` on the undo stack.
    fn move_to_undo(&mut self, inverse: Entry) {
        self.redo.pop();
        self.undo.push_back(inverse);
    }

    /// An undo past the what-if mark ends the what-if.
    fn end_whatif_past_mark(&mut self) {
        if self.whatif.as_ref().and_then(|m| m.depth) > Some(self.undo.len()) {
            self.whatif = None;
        }
    }

    fn push(&mut self, inverse: Entry) {
        self.redo.clear();
        self.undo.push_back(inverse);
        if self.undo.len() > self.cap {
            self.undo.pop_front();
            if let Some(mark) = &mut self.whatif {
                mark.depth = mark.depth.and_then(|depth| depth.checked_sub(1));
            }
        }
    }
}

impl Session {
    /// Apply a person's change and record it so it can be undone.
    ///
    /// [`Session::apply`] is the raw edit operation: it returns its inverse and records nothing. A person's change goes through `edit` instead, which also refuses a typed name the document should not hold (an empty one, a control character, `" / "`). An edit that sets no progress on a task that holds none passes the gate and records nothing.
    ///
    /// # Errors
    /// An [`EditError`] for a refusal of [`Session::apply`] and for a typed name the document should not hold.
    pub fn edit(&mut self, edit: Edit) -> Result<(), EditError> {
        self.gate(&Plan::Edit(&edit))?;
        if self.is_unstarted_zero(&edit) {
            return Ok(());
        }
        self.edit_from(edit, Source::Typed)
    }

    /// [`Session::edit`] for a change `source` made.
    pub(crate) fn edit_from(&mut self, edit: Edit, source: Source) -> Result<(), EditError> {
        // A deleted task is unknown, whatever else is wrong with the edit.
        if let Edit::Move { task, .. }
        | Edit::SetDuration { task, .. }
        | Edit::Rename { task, .. }
        | Edit::Delete { task } = &edit
            && self.schedule.is_deleted(*task)
        {
            return Err(ScheduleError::UnknownTask(*task).into());
        }
        if let Edit::AddLink { pred, succ, .. } = &edit
            && let Some(gone) = [pred, succ]
                .into_iter()
                .find(|t| self.schedule.is_deleted(**t))
        {
            return Err(ScheduleError::UnknownTask(*gone).into());
        }
        edit.check_typed()?;
        let (inverse, _) = self.run_entry(Entry::Step(Step::plain(edit)), false)?;
        self.commit(inverse, source);
        Ok(())
    }

    /// Set the data date to 00:00 on `text`, a day written `YYYY-MM-DD`, or clear it when `text` is
    /// empty: one undo step either way. A set that leaves both the floor and the shown date where they
    /// are (clearing with no data date, or setting the instant already in force) is not an edit: it
    /// recomputes nothing and records no step. Text that is not a day is `EditError::BadDataDate`;
    /// nothing changes on `Err`.
    ///
    /// # Errors
    /// [`EditError::BadDataDate`] for text that is not a day, else the edit's refusal (during a
    /// bar press, for one).
    pub fn set_data_date_text(&mut self, text: &str) -> Result<(), EditError> {
        let text = text.trim();
        let date = if text.is_empty() {
            None
        } else {
            Some(self.origin.parse_day(text).ok_or(EditError::BadDataDate)?)
        };
        if self.data_date == date && self.schedule.data_date() == date {
            return Ok(());
        }
        self.edit(Edit::SetDataDate { date })
    }

    /// Run one history entry. A data-date edit becomes the data dates it replaced, so its inverse
    /// puts back both exactly; any other edit is a step.
    fn run_entry(&mut self, entry: Entry, replay: bool) -> Result<(Entry, f64), EditError> {
        match entry {
            Entry::Step(Step {
                edit: Edit::SetDataDate { date },
                ..
            }) => self.set_data_dates(DataDates::both(date)),
            Entry::Step(step) => self
                .run_step(step, replay)
                .map(|(inverse, ms)| (Entry::Step(inverse), ms)),
            Entry::DataDates(to) => self.set_data_dates(to),
            Entry::Group(steps) => self.run_group(steps, replay),
            Entry::Host(token) => Ok(self.run_host(token)),
        }
    }

    /// Run the steps of a group in order; the entry that puts them back is the inverses, newest
    /// first. A refusal puts back what already ran, so nothing changes on `Err`.
    fn run_group(&mut self, steps: Vec<Step>, replay: bool) -> Result<(Entry, f64), EditError> {
        let mut inverses = Vec::with_capacity(steps.len());
        let mut elapsed = 0.0;
        for step in steps {
            match self.run_step(step, replay) {
                Ok((inverse, ms)) => {
                    inverses.push(inverse);
                    elapsed += ms;
                }
                Err(error) => {
                    for inverse in inverses.into_iter().rev() {
                        let _ = self.run_step(inverse, true);
                    }
                    return Err(error);
                }
            }
        }
        inverses.reverse();
        Ok((Entry::Group(inverses), elapsed))
    }

    /// Apply `edits` in order as one undo step. A refusal puts back what already ran and records
    /// nothing.
    ///
    /// # Errors
    /// The [`EditError`] of the first edit refused.
    pub fn edit_group(&mut self, edits: Vec<Edit>, source: Source) -> Result<(), EditError> {
        self.gate(&Plan::Group(&edits, source))?;
        let steps = edits.into_iter().map(Step::plain).collect();
        let (inverse, _) = self.run_group(steps, false)?;
        self.commit(inverse, source);
        Ok(())
    }

    /// Record one undo step for an action that ran several edits: `inverses` are the inverses
    /// those edits returned, in the order the edits ran. Undo puts them back newest first.
    pub(crate) fn record_group(&mut self, mut inverses: Vec<Step>, source: Source) {
        inverses.reverse();
        self.commit(Entry::Group(inverses), source);
    }

    /// Put the data dates at `to`; the entry that puts them back is what undo or redo keeps.
    fn set_data_dates(&mut self, to: DataDates) -> Result<(Entry, f64), EditError> {
        self.run_set_data_date(to)
            .map(|(before, ms)| (Entry::DataDates(before), ms))
    }

    /// Run one history step: a delete, restore or reparent with the exact meta it carries, any other edit
    /// as `run` does.
    fn run_step(&mut self, step: Step, replay: bool) -> Result<(Step, f64), EditError> {
        match step.edit {
            Edit::Delete { task } => self.run_delete(task, step.meta),
            Edit::Restore { task, place } => self.run_restore(task, place, step.meta, true),
            Edit::Reparent {
                task,
                place,
                siblings,
            } => self.run_reparent(task, place, siblings, step.meta),
            Edit::RestoreLink { dep } if replay => self
                .run_restore_link(dep, true)
                .map(|(inverse, ms)| (Step::plain(inverse), ms)),
            Edit::SetLink { dep, kind, lag } if replay => self
                .run_set_link(dep, kind, lag, true)
                .map(|(inverse, ms)| (Step::plain(inverse), ms)),
            edit => self
                .run(edit)
                .map(|(inverse, ms)| (Step::plain(inverse), ms)),
        }
    }

    /// Undo the latest recorded edit. Returns false when none is available
    /// or while a bar press or a link drag is in flight.
    ///
    /// # Errors
    /// The [`EditError`] of the recorded step's inverse when it is refused; the step stays recorded.
    /// [`EditError::ReadOnly`] while the chart is read-only.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        self.gate(&Plan::Replay("undo", Source::Undo))?;
        self.undo_step()
    }

    /// [`Session::undo`] after the gate has run, for an action that undoes several steps.
    pub(crate) fn undo_step(&mut self) -> Result<bool, EditError> {
        if self.busy() {
            return Ok(false);
        }
        let Some(entry) = self.history.undo.back().cloned() else {
            return Ok(false);
        };
        let row = task_of(&entry);
        let (inverse, _) = self.run_entry(entry, true)?;
        self.commit(inverse, Source::Undo);
        self.history.end_whatif_past_mark();
        self.reveal_changed(row);
        Ok(true)
    }

    /// Redo the latest undone edit. Returns false when none is available
    /// or while a bar press or a link drag is in flight.
    ///
    /// # Errors
    /// The [`EditError`] of the undone step when it is refused; the step stays undone.
    pub fn redo(&mut self) -> Result<bool, EditError> {
        self.gate(&Plan::Replay("redo", Source::Redo))?;
        if self.busy() {
            return Ok(false);
        }
        let Some(entry) = self.history.redo.last().cloned() else {
            return Ok(false);
        };
        let row = task_of(&entry);
        let (inverse, _) = self.run_entry(entry, true)?;
        self.commit(inverse, Source::Redo);
        self.reveal_changed(row);
        Ok(true)
    }

    /// Whether the undo, redo or discard `plan` describes replays a data-date step, which holds no
    /// edit for [`Session::replayed`] and [`Session::discarded`] to list.
    pub(crate) fn replays_data_date(&self, plan: &Plan) -> bool {
        let dates = |entry: Option<&Entry>| matches!(entry, Some(Entry::DataDates(_)));
        match *plan {
            Plan::Replay(_, Source::Undo) => dates(self.history.undo.back()),
            Plan::Replay(..) => dates(self.history.redo.last()),
            Plan::Discard(depth) => self.history.past(depth).any(|entry| dates(Some(entry))),
            _ => false,
        }
    }

    /// After an undo or redo, select the task it changed and bring its bar into view, as a row
    /// click does. The task already selected is left as it is, and so is a link that is still
    /// selected after the step (selecting a row would clear it); a task with no row (deleted, or
    /// hidden by a filter) leaves the selection as the step made it.
    fn reveal_changed(&mut self, task: Option<TaskId>) {
        if let Some(task) = task
            && self.selected_link().is_none()
            && self.selection().map(|selected| selected.task) != Some(task.0)
        {
            self.select_task(task.0);
        }
    }

    /// Whether an edit is available to undo.
    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    /// Whether an edit is available to redo.
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    /// The number of undo and redo entries, respectively.
    pub fn history_len(&self) -> (usize, usize) {
        (self.history.undo.len(), self.history.redo.len())
    }

    /// Record the drag that is ending as one undo step: the inverse holds the
    /// start-no-earlier-than or the duration from before the press. A cancelled drag, a release
    /// before the threshold and a drag that ended where it began record nothing.
    pub(crate) fn commit_drag(&mut self) {
        self.commit_journal();
        if let Some(press) = self.press
            && self.drag_started
            && !press.before.held(&self.schedule, press.task)
        {
            let inverse = press.before.put_back(TaskId(press.task));
            self.commit(Entry::Step(Step::plain(inverse)), Source::Drag);
        }
    }
}
