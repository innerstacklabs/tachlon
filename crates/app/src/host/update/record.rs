//! What a record of an update says of a task's actual dates and remaining work.

use super::{Progress, UpdateError, Values};
use crate::{Session, document::json_read::Value, edit::EditError, host::read::read_duration};
use engine::{Task, TaskId, UnstartedRemaining};

impl Progress {
    pub(super) fn of(task: &Task) -> Self {
        Self {
            start: task.actual_start,
            finish: task.actual_finish,
            remaining: task.remaining,
        }
    }
}

impl Session {
    /// What `task` holds of the values an update sets, and no progress.
    pub(super) fn values_of(&self, task: TaskId) -> Values {
        let current = &self.schedule.tasks()[task.0 as usize];
        Values {
            duration: current.duration,
            snet: current.snet,
            limit: current.limit,
            name: self.meta[task.0 as usize].name.clone(),
            progress: None,
            percent: None,
        }
    }

    /// Set the member `name` of `given`, `actualStart`, `actualFinish` or `remaining`, to what `value` says
    /// (`null` for none), or `None` when `value` is not one.
    pub(super) fn read_actual(
        &self,
        task: TaskId,
        name: &str,
        value: &Value,
        given: &mut Progress,
    ) -> Option<()> {
        let null = matches!(value, Value::Null);
        let date = || {
            (null)
                .then_some(None)
                .or_else(|| (self.read_limit(task, value, name == "actualFinish")).map(Some))
        };
        let minutes = || read_duration(value).map(Some).or(null.then_some(None));
        match name {
            "remaining" => given.remaining = minutes()?,
            "actualFinish" => given.finish = date()?,
            _ => given.start = date()?,
        }
        Some(())
    }

    /// Refuse the actuals `given` that record `at` sets on `task`, with a `remaining` beside `progress` (the
    /// record `said` the members it names), a finish before the start, a finish or remaining with no start (a remaining alone is read when the
    /// schedule's `unstartedRemaining` is on), or a date after the data date, as `validate` and a typed actual do.
    pub(super) fn check_actuals(
        &self,
        at: usize,
        task: TaskId,
        given: Progress,
        said: &[&str],
    ) -> Result<(), UpdateError> {
        let refuse = |kind, member| Err(UpdateError::new(kind, Some(at), Some(member)));
        let unstarted = self.schedule.options().unstarted_remaining == UnstartedRemaining::On;
        let lone = (given.finish.map(|_| "actualFinish"))
            .or(given.remaining.filter(|_| !unstarted).map(|_| "remaining"));
        match (given.start, given.finish, lone) {
            _ if said.contains(&"progress") && said.contains(&"remaining") => {
                return refuse("badValue", "progress");
            }
            (Some(start), Some(finish), _) if finish < start => {
                return refuse(EditError::FinishBeforeStart.kind(), "actualFinish");
            }
            (None, _, Some(member)) => {
                let cleared = said.contains(&"actualStart");
                return refuse("badValue", if cleared { "actualStart" } else { member });
            }
            _ => {}
        }
        for (member, start, finish) in [
            ("actualStart", given.start, None),
            ("actualFinish", None, given.finish),
        ] {
            let late = self.check_actuals_not_after_data_date(task, start, finish);
            late.or_else(|error| refuse(error.kind(), member))?;
        }
        Ok(())
    }
}
