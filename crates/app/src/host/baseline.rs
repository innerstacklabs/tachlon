//! A host's baseline as dates by key (tl#2086): made the baseline a person could set, so the bars, columns and hover card read it.

use super::update::UpdateError;
use crate::Session;
use crate::baseline::{Baseline, BaselineTask};
use crate::document::json_read::{Value, parse};
use crate::edit::{Edit, EditError};
use engine::Instant;
use std::sync::Arc;

/// Why [`Session::set_baseline_records`] set nothing: a record it could not read, or the edit's own refusal.
#[derive(Debug)]
pub enum BaselineRecordsError {
    Record(UpdateError),
    Edit(EditError),
}

impl Session {
    /// Make `json`, an array of `{"key","start","finish"}` records, the baseline: an entry with no float for each keyed
    /// task. A date-only `start` is that day's start, a date-only `finish` its end (decision 0017, rule 6). One undo step.
    ///
    /// # Errors
    ///
    /// [`BaselineRecordsError::Record`] for the first record read wrong: `badJson` (or no records), `noKey`, `unknownKey`,
    /// `duplicateKey`, `badValue` (its `member`) or `finishBeforeStart`, with its position from 0; else the edit's refusal.
    pub fn set_baseline_records(&mut self, json: &str) -> Result<(), BaselineRecordsError> {
        let baseline = self.read_baseline_records(json);
        let baseline = baseline.map_err(BaselineRecordsError::Record)?;
        let baseline = Some(Arc::new(baseline));
        self.edit(Edit::SetBaseline { baseline })
            .map_err(BaselineRecordsError::Edit)
    }

    pub(super) fn read_baseline_records(&self, json: &str) -> Result<Baseline, UpdateError> {
        let fail = |kind, at, member: Option<&str>| UpdateError::new(kind, at, member);
        let root = parse(json).map_err(|_| fail("badJson", None, None))?;
        let records = root.as_array().filter(|records| !records.is_empty());
        let records = records.ok_or_else(|| fail("badJson", None, None))?;
        let mut tasks = vec![None; self.schedule.tasks().len()];
        let mut last = Instant::MIN;
        for (at, record) in records.iter().enumerate() {
            let key = record.get("key").and_then(Value::as_str).unwrap_or("");
            let slot = match self.task_for_key(key) {
                _ if key.is_empty() => return Err(fail("noKey", Some(at), None)),
                None => return Err(fail("unknownKey", Some(at), None)),
                Some(task) => task.0 as usize,
            };
            if tasks[slot].is_some() {
                return Err(fail("duplicateKey", Some(at), None));
            }
            let date = |member, read: fn(&Session, &Value) -> Option<Instant>| {
                let found = record.get(member).and_then(|value| read(self, value));
                found.ok_or_else(|| fail("badValue", Some(at), Some(member)))
            };
            let es = date("start", Session::read_date)?;
            let ef = date("finish", Session::read_finish)?;
            // A day's end is never a start: a date-only finish that only reaches the start is the day before it.
            let day = record
                .get("finish")
                .and_then(Value::as_str)
                .is_some_and(|t| !t.contains('T'));
            if ef < es || (ef == es && day) {
                return Err(fail("finishBeforeStart", Some(at), None));
            }
            tasks[slot] = Some(BaselineTask {
                es,
                ef,
                float: None,
            });
            last = last.max(ef);
        }
        Ok(Baseline::from_parts(tasks, last))
    }
}
