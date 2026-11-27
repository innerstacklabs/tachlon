//! Edits: every change a person makes to the schedule or its names goes
//! through [`Session::apply`] as one [`Edit`], which returns the edit that
//! undoes it. Undo (gs#51) then needs no new plumbing: it applies the
//! inverse it was handed.
//!
//! Insert, delete and link variants (gs#49, gs#50). Variants
//! carry a [`TaskId`] (decision 0001), never a row.

mod duration;
mod error;
pub mod fields;
pub mod finish;
mod limit;
mod link_text;
mod links;
mod percent;
mod physical;
mod predecessors;
mod progress;
pub mod start;

use crate::{Session, baseline::Baseline, meta::TaskMeta, reparent::Siblings, structure::Place};
pub use duration::{
    DurationError, day_minutes, format_duration, format_float_days, parse_duration,
    parse_duration_cell,
};
use engine::{DepId, Instant, Limit, LinkType, Minutes, ScheduleError, TaskId};
pub use error::EditError;
pub use link_text::{LinkTextError, format_link_text, parse_link_text};
pub(crate) use physical::percent_number;
pub(crate) use predecessors::Memo as PredecessorsMemo;
pub use predecessors::PredecessorFault;
use std::sync::Arc;

/// One change to the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Set or clear a task's start-no-earlier-than (a bar drag).
    Move { task: TaskId, snet: Option<Instant> },
    /// Set a task's duration, in working minutes of its calendar. Zero makes
    /// a milestone.
    SetDuration { task: TaskId, minutes: Minutes },
    /// Rename a task: `name` replaces its own name. Its descendants keep theirs, and their outline
    /// paths show the new name (`Level 3 / Concrete / Pour slab` renamed `Slab` is
    /// `Level 3 / Concrete / Slab`).
    Rename { task: TaskId, name: String },
    /// Add a task as a leaf at `place`, named `name`, 1 working day of the calendar of
    /// `place.after` (else the shading calendar). It takes the next free id and is never an
    /// inverse. Refused under a row filter or lookahead window. Inverse: Delete.
    Insert { place: Place, name: String },
    /// Tombstone a leaf and take it out of the outline. Refused for a summary. Inverse: Restore.
    Delete { task: TaskId },
    /// Bring a deleted task back at `place`; its outline level follows the parent `place` names.
    /// Inverse: Delete. Undo puts back the exact parent and level from before the step, which
    /// history keeps privately: no caller can set a level.
    Restore { task: TaskId, place: Place },
    /// Move a live task with its subtree to `place`, shifting the subtree's levels by one delta.
    /// `siblings` also adopts the following siblings as the task's last children (`Adopt`) or
    /// hands its last children back as its following siblings (`Release`). Refused under a row
    /// filter or lookahead window and for a leaf with a link that would become a summary.
    /// Inverse: Reparent to the old place. Undo puts back the exact parent, level and position.
    Reparent {
        task: TaskId,
        place: Place,
        siblings: Siblings,
    },
    /// Link `pred` to `succ` (a new slot, never an inverse). Refused for a summary, a loop, a
    /// second live link on the pair, or a deleted end. Inverse: `RemoveLink`.
    AddLink {
        pred: TaskId,
        succ: TaskId,
        kind: LinkType,
        lag: Minutes,
    },
    /// Take a link out of force; it keeps its slot. Inverse: `RestoreLink`.
    RemoveLink { dep: DepId },
    /// Bring a removed link back as it was, checked like `AddLink`. Inverse: `RemoveLink`.
    RestoreLink { dep: DepId },
    /// Change a live link's type and lag (gs#703), keeping its slot and its lag calendar. Refused
    /// like `AddLink` for a summary end, a busy session and a second live link on the pair with the
    /// new type, and for a removed or unknown link. Inverse: `SetLink` with the type and lag it
    /// replaced.
    SetLink {
        dep: DepId,
        kind: LinkType,
        lag: Minutes,
    },
    /// Set the baseline (gs#703): `baseline` is put in force as it is, or, when `None`, a snapshot
    /// of the dates now. Inverse: `SetBaseline` of the baseline it replaced, else `ClearBaseline`.
    /// Schedule dates, history of other edits and the outline do not change.
    SetBaseline { baseline: Option<Arc<Baseline>> },
    /// Clear the baseline. Refused when there is none. Inverse: `SetBaseline` of the one it removed.
    ClearBaseline,
    /// Set or clear the data date (gs#703): unstarted work starts no earlier than it, so the
    /// whole schedule recomputes. Refused during a bar press. Only [`Session::edit`] takes it:
    /// history keeps the schedule's own floor with the date the scene marks, so undo restores both
    /// exactly, and [`Session::apply`] refuses it (gs#755).
    SetDataDate { date: Option<Instant> },
    /// Record a task's progress (gs#703, decision 0008): set or clear its actual start, actual
    /// finish and remaining duration together, then recompute in full. Refused for a summary and
    /// a busy session, for an actual finish before the actual start, and for a negative
    /// remaining duration. An actual finish with no actual start is allowed, as the engine's
    /// setters allow it. Inverse: `SetProgress` with the three fields it replaced, `None`s included.
    SetProgress {
        task: TaskId,
        actual_start: Option<Instant>,
        actual_finish: Option<Instant>,
        remaining: Option<Minutes>,
    },
    /// Replace a task's date limits, its pin and as-late-as-possible included (gantt#1875), then
    /// recompute in full. Inverse: `SetLimit` with the replaced limits.
    SetLimit { task: TaskId, limit: Limit },
    /// Set or clear a task's physical percent complete, in hundredths of a percent (tl#2080,
    /// decision 0008 item 1a). It is metadata: no date moves and nothing recomputes. Refused for a
    /// summary. Inverse: `SetPhysical` with the value it replaced.
    SetPhysical { task: TaskId, physical: Option<u16> },
}

impl Session {
    /// Apply `edit` and return the edit that undoes it. A rename changes the
    /// session's task metadata. On `Err` nothing has changed.
    ///
    /// A move recomputes through the cone, and so does a duration change
    /// ([`engine::Schedule::recompute_duration_cone_moved`]), which also
    /// redoes the task's own late dates and its predecessors' float where its
    /// finish does not move. The duration cone gives up to a full
    /// [`engine::Schedule::compute`] when it is wide or the project finish
    /// moves; the move cone reruns its backward pass when the finish moves. A rename
    /// recomputes nothing.
    /// Summary dates are rolled up whenever an edit changes the solution.
    ///
    /// Every applied edit bumps [`Session::revision`] and [`Session::stamp`].
    ///
    /// A rename is stored as given: `apply` does not hold a name to the rules of typed text, so
    /// the inverse of a rename restores an imported name that holds `" / "` byte for byte. A
    /// person's rename goes through [`Session::edit`], which checks the name first.
    ///
    /// # Errors
    /// An [`EditError`] naming why the edit is refused: an unknown task, a busy session, or the
    /// engine's refusal.
    pub fn apply(&mut self, edit: Edit) -> Result<Edit, EditError> {
        self.run(edit).map(|(inverse, _)| inverse)
    }

    /// [`Session::apply`], and the milliseconds the recompute took (zero for
    /// a rename). The drag reads the time for its overlay.
    #[expect(clippy::too_many_lines, reason = "one arm per edit kind")]
    pub(crate) fn run(&mut self, edit: Edit) -> Result<(Edit, f64), EditError> {
        match edit {
            Edit::Move { task, snet } => {
                let before = self.task_snet(task)?;
                let t = &self.schedule.tasks()[task.0 as usize];
                if t.actual_start.is_some() || t.actual_finish.is_some() {
                    return Err(EditError::HasActuals);
                }
                self.schedule.set_snet(task, snet)?;
                self.remember_critical(Some(task));
                let old_finish = self.solution.project_finish;
                let t0 = (self.clock)();
                let result =
                    self.schedule
                        .recompute_cone_moved(&mut self.solution, task, &mut self.moved);
                let elapsed = (self.clock)() - t0;
                match result {
                    Ok(()) => {
                        self.rollup_moved_summaries();
                        self.follow_filter();
                        let held = self.reports.held;
                        self.changed();
                        self.note_moved(task, old_finish, held);
                        Ok((Edit::Move { task, snet: before }, elapsed))
                    }
                    Err(error) => {
                        self.forget_critical();
                        let _ = self.schedule.set_snet(task, before);
                        Err(error.into())
                    }
                }
            }
            Edit::SetDuration { task, minutes } => {
                let before = self
                    .schedule
                    .tasks()
                    .get(task.0 as usize)
                    .ok_or(ScheduleError::UnknownTask(task))?
                    .duration;
                self.schedule.set_duration(task, minutes)?;
                self.remember_critical(Some(task));
                // The cone is exact only when `solution` equals `compute()` before this
                // one change (`recompute_duration_cone_moved`). It does: every other path
                // that changes the schedule (a move through the cone, an insert, delete,
                // restore or link edit through a full compute) leaves the solution equal to
                // `compute()`, and this edit changes one duration, undone on `Err`.
                let old_finish = self.solution.project_finish;
                let t0 = (self.clock)();
                let result = self.schedule.recompute_duration_cone_moved(
                    &mut self.solution,
                    task,
                    &mut self.moved,
                );
                let elapsed = (self.clock)() - t0;
                match result {
                    Ok(_) => {
                        self.rollup_moved_summaries();
                        self.follow_filter();
                        let held = self.reports.held;
                        self.changed();
                        self.note_moved(task, old_finish, held);
                        let inverse = Edit::SetDuration {
                            task,
                            minutes: before,
                        };
                        Ok((inverse, elapsed))
                    }
                    Err(error) => {
                        self.forget_critical();
                        let _ = self.schedule.set_duration(task, before);
                        Err(error.into())
                    }
                }
            }
            Edit::Insert { place, name } => self
                .run_insert(place, &name, true)
                .map(|(inverse, ms)| (inverse.edit, ms)),
            Edit::Delete { task } => self
                .run_delete(task, None)
                .map(|(inverse, ms)| (inverse.edit, ms)),
            Edit::Restore { task, place } => self
                .run_restore(task, place, None, true)
                .map(|(inverse, ms)| (inverse.edit, ms)),
            Edit::Reparent {
                task,
                place,
                siblings,
            } => self
                .run_reparent(task, place, siblings, None, false)
                .map(|(inverse, ms)| (inverse.edit, ms)),
            Edit::AddLink {
                pred,
                succ,
                kind,
                lag,
            } => self.run_add_link(pred, succ, kind, lag),
            Edit::RemoveLink { dep } => self.run_remove_link(dep),
            Edit::RestoreLink { dep } => self.run_restore_link(dep, false),
            Edit::SetLink { dep, kind, lag } => self.run_set_link(dep, kind, lag, false),
            Edit::SetBaseline { baseline } => Ok(self.run_set_baseline(baseline)),
            Edit::ClearBaseline => self.run_clear_baseline(),
            Edit::SetDataDate { .. } => Err(EditError::DataDateThroughEdit),
            Edit::SetProgress {
                task,
                actual_start,
                actual_finish,
                remaining,
            } => self.run_set_progress(task, actual_start, actual_finish, remaining),
            Edit::SetLimit { task, limit } => self.run_set_limit(task, limit),
            Edit::SetPhysical { task, physical } => self.run_set_physical(task, physical),
            Edit::Rename { task, name } => {
                if self.schedule.is_deleted(task) {
                    return Err(ScheduleError::UnknownTask(task).into());
                }
                let old = rename(&mut self.meta, task, &name)?;
                self.changed();
                Ok((Edit::Rename { task, name: old }, 0.0))
            }
        }
    }

    fn task_snet(&self, task: TaskId) -> Result<Option<Instant>, ScheduleError> {
        self.schedule
            .tasks()
            .get(task.0 as usize)
            .map(|task| task.snet)
            .ok_or(ScheduleError::UnknownTask(task))
    }

    /// An edit was applied: the table has new rows to show.
    pub(crate) fn changed(&mut self) {
        self.revision = self.revision().wrapping_add(1);
        self.stamp = self.stamp().wrapping_add(1);
        self.selection_stale |= self.selection.is_some();
        // Any change replaces the last report; `note_moved` makes the new one after this, and
        // hands it the hold the pointer had on the old one. A change that makes no report ends it.
        self.reports.report = None;
        self.reports.held = None;
        // The moved filter ends with the report it read; `settle_moved_filter` turns it back on
        // for the report this change makes, if any.
        if self.row_filter == crate::RowFilter::Moved {
            self.row_filter = crate::RowFilter::Off;
            self.reports.reopen = true;
            if self.busy() {
                self.rows_stale = true;
            } else {
                self.apply_rows();
            }
        } else if !self.reports.journal_on() {
            self.reports.reopen = false;
        }
    }
}

impl Edit {
    /// Whether a person may make this edit: progress dates in order, a typed name needs a visible character and may not
    /// hold the separator `" / "` or a control character. A name that starts with `"/ "` or ends
    /// with `" /"` would join the separator in the outline path, so it reads as a different last
    /// segment: both edges are refused too. Every other edit passes; the schedule judges those.
    pub(crate) fn check_typed(&self) -> Result<(), EditError> {
        // New progress input: an actual finish before the actual start. The check is not in
        // `run`, because the engine accepts such dates (an import can hold them) and the inverse
        // of an edit must put them back, which `run` would otherwise refuse.
        if let Self::SetProgress {
            actual_start: Some(start),
            actual_finish: Some(finish),
            ..
        } = self
            && finish < start
        {
            return Err(EditError::FinishBeforeStart);
        }
        let (Self::Rename { name, .. } | Self::Insert { name, .. }) = self else {
            return Ok(());
        };
        if name.trim().is_empty() {
            return Err(EditError::EmptyName);
        }
        if name.contains(" / ")
            || name.chars().any(char::is_control)
            || name.starts_with("/ ")
            || name.ends_with(" /")
        {
            return Err(EditError::InvalidName);
        }
        Ok(())
    }
}

/// Replace `task`'s own name with `name`, stored exactly as given so the returned inverse restores
/// a padded name byte for byte; callers that take typed text trim it first. Its descendants keep
/// their names: each shows the new name through its derived outline path. Returns the old name.
fn rename(meta: &mut [TaskMeta], task: TaskId, name: &str) -> Result<String, EditError> {
    let entry = meta
        .get_mut(task.0 as usize)
        .ok_or(EditError::UnknownTask(task))?;
    Ok(std::mem::replace(&mut entry.name, name.to_string()))
}
