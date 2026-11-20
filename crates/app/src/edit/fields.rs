//! A task's changed fields as one undo step (gantt#1875), each planned with `plan_*_text`.

use super::{Edit, EditError, parse_duration_cell};
use crate::Session;
use crate::history::{Source, Step};
use crate::host::Plan;
use crate::texts::{PERCENT_ALONE, Worded};
use engine::TaskId;

/// The fields the dialog sends and the label a refusal names, in the order their edits run: a name
/// first, since an edit that moves nothing ends the change summary of the one before it.
pub const TASK_FIELDS: [(&str, &str); 12] = [
    ("name", "Name"),
    ("duration", "Duration"),
    ("start", "Start"),
    ("finish", "Finish"),
    ("snlt", "Start no later than"),
    ("fnet", "Finish no earlier than"),
    ("fnlt", "Finish no later than"),
    ("percent", "Percent"),
    ("actual_start", "Actual start"),
    ("actual_finish", "Actual finish"),
    ("remaining", "Remaining"),
    ("predecessors", "Predecessors"),
];

impl Session {
    fn said<E: Worded>(&self) -> impl Fn(E) -> String + '_ {
        move |error| self.say(&error)
    }

    /// The edit the dialog's field `key` makes of `task` for `typed`, planned against what the session holds now.
    fn plan_field(&self, task: TaskId, key: &str, typed: &str) -> Result<Option<Edit>, String> {
        Ok(match key {
            "name" => Some(Edit::Rename {
                task,
                name: typed.trim().into(),
            }),
            "duration" => {
                let minutes =
                    parse_duration_cell(typed, &self.schedule, task).map_err(self.said())?;
                Some(Edit::SetDuration { task, minutes })
            }
            "start" => self.plan_start_text(task, typed).map_err(self.said())?.1,
            "finish" => self.plan_finish_text(task, typed).map_err(self.said())?,
            "snlt" | "fnet" | "fnlt" => Some(
                self.plan_limit_text(task, key, typed)
                    .map_err(self.said())?,
            ),
            "percent" => Some(self.plan_percent_text(task, typed).map_err(self.said())?)
                .filter(|edit| !self.is_unstarted_zero(edit)),
            _ => Some(
                self.plan_progress_field_text(task, key, typed)
                    .map_err(self.said())?,
            ),
        })
    }

    /// The edits the dialog's field `key` makes of `task` for `typed`: a predecessors list is one per link it changes.
    fn plan_edits(&self, task: TaskId, key: &str, typed: &str) -> Result<Vec<Edit>, String> {
        if key == "predecessors" {
            return self
                .plan_predecessors_text(task, typed)
                .map_err(self.said());
        }
        Ok(self.plan_field(task, key, typed)?.into_iter().collect())
    }

    /// The start `task` has once `edits` are applied; `None` while none of them moves it.
    fn start_after(&self, task: TaskId, edits: &[Edit]) -> Option<i64> {
        let snet = edits.iter().rev().find_map(|edit| match edit {
            Edit::Move { task: moved, snet } if *moved == task => Some(*snet),
            _ => None,
        })?;
        let mut ahead = self.schedule.clone();
        ahead.set_snet(task, snet).ok()?;
        Some(ahead.compute().ok()?.dates[task.0 as usize].es)
    }

    /// The edits `text` asks of `task`, in the order they run, a finish measured from the start
    /// the edits before it leave; a text its plan refuses is left out.
    pub(crate) fn dialog_edits<'t>(
        &self,
        task: TaskId,
        text: &dyn Fn(&str) -> Option<&'t str>,
    ) -> Vec<Edit> {
        let mut edits = Vec::new();
        for &(key, _) in TASK_FIELDS.iter().filter(|_| self.is_live(task)) {
            let Some(typed) = text(key) else { continue };
            let planned = if key == "finish" {
                let start = self.start_after(task, &edits);
                let finish = self.plan_finish_from(task, typed, start);
                finish
                    .map(|edit| edit.into_iter().collect())
                    .map_err(self.said())
            } else {
                self.plan_edits(task, key, typed)
            };
            edits.extend(planned.unwrap_or_default());
        }
        edits
    }

    /// Apply the changed `fields` of `task`, `(key, text)` pairs with keys from [`TASK_FIELDS`], as
    /// one undo step, each planned against the state the earlier ones left; a key not in it is ignored. A percent sets the duration's share, the actual
    /// dates and the remaining, so it is refused beside any of them.
    ///
    /// # Errors
    /// A message starting with the label of the field refused (`Duration: ...`), for text its plan
    /// refuses or a name the document should not hold; nothing changes then.
    pub fn set_task_fields(
        &mut self,
        task: TaskId,
        fields: &[(&str, impl AsRef<str>)],
    ) -> Result<(), String> {
        let text = |key: &str| fields.iter().find(|f| f.0 == key).map(|f| f.1.as_ref());
        self.gate(&Plan::Dialog(task, &text)).map_err(self.said())?;
        let alone = ["duration", "actual_start", "actual_finish", "remaining"];
        if !self.is_live(task) {
            return Err(self.say(&EditError::UnknownTask(task)));
        } else if text("percent").is_some() && alone.iter().any(|key| text(key).is_some()) {
            return Err(format!("Percent: {}", self.texts.say(&PERCENT_ALONE, &[])));
        }
        let mut inverses: Vec<Edit> = Vec::new();
        let applied = TASK_FIELDS.iter().try_for_each(|&(key, label)| {
            let Some(typed) = text(key) else {
                return Ok(());
            };
            // Planned against the state the earlier fields left: a finish is measured from the
            // start just set, a percent from the duration just set.
            let planned = self.plan_edits(task, key, typed);
            for edit in planned.map_err(|e| format!("{label}: {e}"))? {
                edit.check_typed()
                    .map_err(|e| format!("{label}: {}", self.say(&e)))?;
                inverses.push(self.run(edit.clone()).map_err(self.said())?.0);
            }
            Ok::<(), String>(())
        });
        if let Err(error) = applied {
            for inverse in inverses.into_iter().rev() {
                let _ = self.run(inverse);
            }
            return Err(error);
        }
        if !inverses.is_empty() {
            let steps = inverses.into_iter().map(Step::plain).collect();
            self.record_group(steps, Source::Dialog);
        }
        if let Some(typed) =
            text("percent").and_then(|t| t.trim_end_matches('%').trim().parse().ok())
        {
            self.typed_percent.insert(task.0, typed);
        }
        Ok(())
    }
}
