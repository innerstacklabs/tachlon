//! A proposed actual (gantt#2093): one field of a task's progress, held as a report that is planned again as an
//! `Edit::SetProgress` against the task as it is whenever the held changes are solved or accepted.

use super::Proposal;
use crate::edit::{Edit, EditError};
use crate::{Session, what_moves::refuse};
use engine::{Instant, Schedule, ScheduleError, Solution, TaskId};

/// A held report: the task, the field and the text as they were proposed.
pub(super) type Report = (TaskId, String, String);

impl Session {
    /// Propose `text`, written as its cell writes it, for `field` of `task`'s progress (`actual_start`, `actual_finish`,
    /// `remaining` or `percent`), held as [`Session::propose_move`] holds a move. A date alone is the first working
    /// minute of the day for a start and the end of its last for a finish. A started task is allowed; the refusals add
    /// those of a typed progress edit (`badDate`, `badPercent`, ...).
    pub fn propose_progress(&mut self, task: u32, field: &str, text: &str) -> String {
        if let Some(refusal) = self.ask(&[task], false) {
            return refusal;
        }
        let task = TaskId(task);
        match self.plan_report(task, field, text) {
            Ok(edit) => {
                let said = self.hold(edit);
                if said.starts_with(r#"{"ok":true"#)
                    && let Some(held) = &mut self.proposal
                {
                    held.reports.retain(|report| report.0 != task);
                    held.reports.push((task, field.into(), text.into()));
                }
                said
            }
            Err(error) => refuse(error.kind(), &self.say(&error)),
        }
    }

    /// The edit that reports `text` for `field` of `task`, planned against the task as it is now.
    fn plan_report(&self, task: TaskId, field: &str, text: &str) -> Result<Edit, EditError> {
        match field {
            "percent" => self.plan_percent_text(task, text),
            _ => self.plan_progress_field_text(task, field, &self.dated(task, field, text)),
        }
    }

    /// `text` with the time of day a date alone takes (decision 0017, rule 6), on the task's calendar.
    fn dated(&self, task: TaskId, field: &str, text: &str) -> String {
        let text = text.trim();
        let Some(day) = self.origin.parse_day(text) else {
            return text.to_string();
        };
        let at = match field {
            "actual_start" => self.day_edge(task, day, false),
            "actual_finish" => self.day_edge(task, day, true),
            _ => return text.to_string(),
        };
        self.date_text(at)
    }

    /// `edit`, planned again from its report in `reports` against the live task; any other edit as it is.
    pub(super) fn live_edit(&self, reports: &[Report], edit: &Edit) -> Result<Edit, EditError> {
        let Edit::SetProgress { task, .. } = edit else {
            return Ok(edit.clone());
        };
        match reports.iter().find(|report| report.0 == *task) {
            Some((_, field, text)) => self.plan_report(*task, field, text),
            None => Ok(edit.clone()),
        }
    }

    /// Each held change as [`Session::live_edit`] plans it, or the refusal that stops it.
    pub(super) fn live_edits<'a>(
        &'a self,
        held: &'a Proposal,
    ) -> impl Iterator<Item = Result<Edit, EditError>> + 'a {
        let plan = |edit| self.live_edit(&held.reports, edit);
        held.edits.iter().map(plan)
    }

    /// A ghost for each task the changes move, and for each task whose progress they report, though no date moves.
    pub(super) fn ghosts_of(
        &self,
        solved: &Solution,
        edits: &[Edit],
    ) -> Vec<(u32, Instant, Instant)> {
        let at = |task: TaskId| {
            let now = &solved.dates[task.0 as usize];
            (task.0, now.es, now.ef)
        };
        let reported = edits.iter().filter_map(|edit| match edit {
            Edit::SetProgress { task, .. } => Some(*task),
            _ => None,
        });
        let mut tasks: Vec<TaskId> = self.moved_tasks(solved).chain(reported).collect();
        tasks.sort_by_key(|task| task.0);
        tasks.dedup();
        tasks.into_iter().map(at).collect()
    }
}

/// Make a held `Edit::SetProgress` in `copy`, a copy of the schedule; any other edit changes nothing.
pub(super) fn solve(copy: &mut Schedule, edit: &Edit) -> Result<(), ScheduleError> {
    let Edit::SetProgress {
        task,
        actual_start,
        actual_finish,
        remaining,
    } = *edit
    else {
        return Ok(());
    };
    copy.set_remaining(task, remaining)?;
    copy.set_actual_start(task, actual_start)?;
    copy.set_actual_finish(task, actual_finish)
}
