//! A host's new values for many tasks, by key, as one recompute (decision 0019, rule 13). It is not an
//! edit: no gate, no undo step, no outbox commit, and `revision` stays; `stamp` rises, so the table
//! redraws. Every record is read before a value is written, and a refusal puts every value back.

use super::read::read_duration;
use crate::{
    Session,
    document::json_read::{Value, parse},
    edit::{Edit, EditError},
    history::Step,
    json::quote,
};
use engine::{Instant, Limit, Minutes, TaskId};
use std::collections::HashSet;

/// How many tasks an update changed and how many times it solved: twice if it mixes progress with timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updated {
    pub tasks: usize,
    pub solves: usize,
    /// One JSON object (`record`, `key`, `member`, `host`, `chart`) per `hostStart` or `hostFinish` the chart computed otherwise.
    pub differs: Vec<String>,
}

/// Why an update was refused. Nothing changed. `kind` is `badJson`, `noKey`, `unknownKey` (a
/// deleted task's key too), `duplicateKey`, `notUpdatable` (a member an update does not set, or a summary),
/// `badValue`, or an [`EditError::kind`]: `busy` and `schedule`, and for a `name` or a `progress` the
/// reasons a typed rename or percent gets (`emptyName`, `invalidName`, `badPercent`, `actualAfterDataDate`).
/// `record` counts from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateError {
    pub kind: &'static str,
    pub record: Option<usize>,
    pub member: Option<String>,
}

impl UpdateError {
    pub(super) fn new(kind: &'static str, record: Option<usize>, member: Option<&str>) -> Self {
        let member = member.map(str::to_owned);
        Self {
            kind,
            record,
            member,
        }
    }
}

/// What an update sets on one task. A record with no `progress` leaves `progress` and `percent` empty,
/// and the task's actual dates and remaining work are not written.
#[derive(Clone)]
pub(super) struct Values {
    duration: Minutes,
    snet: Option<Instant>,
    limit: Limit,
    name: String,
    progress: Option<Progress>,
    percent: Option<u8>,
    host: Vec<(usize, String, Instant)>,
}

/// A task and what a record sets on it, beside what it held.
pub(super) type Written = (TaskId, (Values, Values));

/// A task's actual start, actual finish and remaining work: the three a percent sets.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Progress {
    start: Option<Instant>,
    finish: Option<Instant>,
    remaining: Option<Minutes>,
}

impl Values {
    /// The steps that put `old` back where `self` is, one for each of the name, duration, start floor, limits
    /// and progress that differ, in the order a call sets them: an undo runs them newest first.
    pub(super) fn steps_back(&self, old: &Values, task: TaskId) -> Vec<Step> {
        let was = self.progress.zip(old.progress).filter(|(a, b)| a != b);
        [
            (self.name != old.name).then(|| Edit::Rename {
                task,
                name: old.name.clone(),
            }),
            (self.duration != old.duration).then(|| Edit::SetDuration {
                task,
                minutes: old.duration,
            }),
            (self.snet != old.snet).then(|| Edit::Move {
                task,
                snet: old.snet,
            }),
            (self.limit != old.limit).then(|| Edit::SetLimit {
                task,
                limit: old.limit,
            }),
            was.map(|(_, p)| Edit::SetProgress {
                task,
                actual_start: p.start,
                actual_finish: p.finish,
                remaining: p.remaining,
            }),
        ]
        .into_iter()
        .flatten()
        .map(Step::plain)
        .collect()
    }
}

impl Session {
    /// Set the durations, starts, names and progress that `json`, an array of records, gives, each record
    /// naming its task by `key`. `duration` is a whole number of working minutes; `start`
    /// is held as `snet` unless the record has an `snet`; `start`, `snet` and the limits `snlt` (start no later than), `fnet` and `fnlt` (finish no earlier and no later than; a date alone is the end of that day) take a date or `null`, and the limits are set as a typed limit sets them. `name` is trimmed
    /// and checked as a typed rename is. `progress` is a whole percent from 0 to 100, set as a typed percent
    /// sets it, and refused as a typed actual is when an actual it sets is after the data date. The call
    /// applies every record's `duration`, `start`, `snet`, limits and `name` first and works each `progress` out
    /// against the plan those leave, as the typed dialog does field by field: a call that mixes progress
    /// with a duration, start, snet or limit in any record solves twice, and any other call once. A member a
    /// record leaves out keeps its value. `hostStart` and `hostFinish`, the dates the host computed, set nothing and are named in `differs` when the chart's early start or finish is not theirs (a day alone compares the day, a finish's day being its last minute's). Every other member, the pins and `alap` included, is `notUpdatable`.
    ///
    /// # Errors
    ///
    /// An [`UpdateError`]; the schedule, the selection, the history and `revision` are then as they were.
    pub fn update_tasks(&mut self, json: &str) -> Result<Updated, UpdateError> {
        let revision = self.revision;
        let done = self.apply_records(json, false);
        self.revision = revision;
        done.map(|(updated, _)| updated)
    }

    /// [`Session::update_tasks`]'s work, answering the records written beside what each held, for
    /// [`Session::host_edit`]. With `undoable` a call that changes the start floor of a task with actuals
    /// is refused as a typed move is, because the step that puts the floor back would be.
    pub(super) fn apply_records(
        &mut self,
        json: &str,
        undoable: bool,
    ) -> Result<(Updated, Vec<Written>), UpdateError> {
        let refuse = |error: EditError| UpdateError::new(error.kind(), None, None);
        if self.busy() {
            return Err(refuse(EditError::Busy));
        }
        let bad_json = || UpdateError::new("badJson", None, None);
        let root = parse(json).map_err(|_| bad_json())?;
        let records = root.as_array().ok_or_else(bad_json)?;
        let mut planned = Vec::with_capacity(records.len());
        let mut seen = HashSet::new();
        for (at, record) in records.iter().enumerate() {
            let key = record.get("key").and_then(Value::as_str).unwrap_or("");
            let fail = |kind| UpdateError::new(kind, Some(at), None);
            let task = match self.task_for_key(key) {
                _ if key.is_empty() => return Err(fail("noKey")),
                None => return Err(fail("unknownKey")),
                Some(task) if !seen.insert(task.0) => return Err(fail("duplicateKey")),
                Some(task) => task,
            };
            planned.push((task, self.read_values(at, task, record)?));
        }
        let progress = planned.iter().any(|(_, (new, _))| new.percent.is_some());
        let mixed = progress
            && records.iter().any(|record| {
                ["duration", "start", "snet", "snlt", "fnet", "fnlt"]
                    .iter()
                    .any(|member| record.get(member).is_some())
            });
        let mut done = Vec::with_capacity(planned.len());
        for (at, (task, (new, old))) in planned.iter().enumerate() {
            done.push((*task, old.clone()));
            let t = &self.schedule.tasks()[task.0 as usize];
            let started = t.actual_start.or(t.actual_finish).is_some();
            let refused =
                (undoable && started && new.snet != old.snet).then_some(EditError::HasActuals);
            if let Some(error) = refused.or_else(|| self.put(*task, new).err()) {
                self.put_back(&done);
                return Err(UpdateError::new(error.kind(), Some(at), None));
            }
        }
        if progress {
            let applied = self.work_out_progress(&mut planned, mixed).and_then(|()| {
                (planned.iter())
                    .try_for_each(|(task, (new, _))| self.put(*task, new).map_err(refuse))
            });
            if let Err(error) = applied {
                self.put_back(&done);
                return Err(error);
            }
        }
        self.solve_full(None, |session| session.put_back(&done))
            .map_err(refuse)?;
        if let Some(held) = &mut self.proposal {
            held.solved = None;
        }
        for (task, (new, _)) in &planned {
            if let Some(percent) = new.percent {
                self.typed_percent.insert(task.0, percent);
            }
        }
        let updated = Updated {
            tasks: planned.len(),
            solves: 1 + usize::from(mixed),
            differs: self.differences(records, &planned),
        };
        Ok((updated, planned))
    }

    /// Each `hostStart` and `hostFinish` in `planned` that is not its task's early start or finish, as JSON.
    fn differences(&self, records: &[Value], planned: &[Written]) -> Vec<String> {
        let mut found = vec![];
        for (record, (task, (new, _))) in planned.iter().enumerate() {
            let d = &self.solution.dates[task.0 as usize];
            let key = records[record].get("key").and_then(Value::as_str);
            for (kind, text, host) in &new.host {
                let (chart, back) = [(d.es, 0), (d.ef, 1)][*kind];
                let day = |t| self.origin.day_text(t - back);
                if *host != chart && (text.contains('T') || day(*host) != day(chart)) {
                    let shown = [day(chart), self.date_text(chart).replacen(' ', "T", 1)];
                    found.push(format!(
                        r#"{{"record":{record},"key":{},"member":"{}","host":{},"chart":{}}}"#,
                        quote(key.unwrap_or_default()),
                        ["start", "finish"][*kind],
                        quote(text),
                        quote(&shown[usize::from(text.contains('T'))])
                    ));
                }
            }
        }
        found
    }

    /// Set each record's `progress` in `planned`: worked out against the plan the other members leave, which
    /// is solved apart from the session when `mixed`, else the plan the session holds.
    fn work_out_progress(
        &mut self,
        planned: &mut [Written],
        mixed: bool,
    ) -> Result<(), UpdateError> {
        let schedule_error = |e| UpdateError::new(EditError::from(e).kind(), None, None);
        let first = mixed
            .then(|| self.schedule.compute().map_err(schedule_error))
            .transpose()?;
        let dates = first.as_ref().map_or(&self.solution.dates, |s| &s.dates);
        for (at, (task, (new, _))) in planned.iter_mut().enumerate() {
            let Some(percent) = new.percent else {
                continue;
            };
            let (start, finish, remaining) =
                self.percent_progress(dates, *task, i64::from(percent), new.duration);
            self.check_actuals_not_after_data_date(*task, start, finish)
                .map_err(|error| UpdateError::new(error.kind(), Some(at), Some("progress")))?;
            new.progress = Some(Progress {
                start,
                finish,
                remaining,
            });
        }
        Ok(())
    }

    /// The values `record` gives `task` and the ones it has now.
    fn read_values(
        &self,
        at: usize,
        task: TaskId,
        record: &Value,
    ) -> Result<(Values, Values), UpdateError> {
        let current = &self.schedule.tasks()[task.0 as usize];
        let mut old = Values {
            duration: current.duration,
            snet: current.snet,
            limit: current.limit,
            name: self.meta[task.0 as usize].name.clone(),
            progress: None,
            percent: None,
            host: vec![],
        };
        let members: &[_] = match record {
            Value::Object(members)
                if !self.outline.as_ref().is_some_and(|o| o.is_summary(task)) =>
            {
                members
            }
            _ => return Err(UpdateError::new("notUpdatable", Some(at), Some("key"))),
        };
        let mut new = old.clone();
        let mut percent = None;
        for (name, value) in members {
            let bad = || UpdateError::new("badValue", Some(at), Some(name));
            let refused = |error: EditError| UpdateError::new(error.kind(), Some(at), Some(name));
            let date = |value: &Value| match value {
                Value::Null => Ok(None),
                _ => self.read_date(value).map(Some).ok_or_else(bad),
            };
            let finish = |value: &Value| match value {
                Value::Null => Ok(None),
                _ => self.read_finish(value).map(Some).ok_or_else(bad),
            };
            match &**name {
                "key" => {}
                "duration" => new.duration = read_duration(value).ok_or_else(bad)?,
                "start" if record.get("snet").is_some() => {
                    date(value)?;
                }
                "start" | "snet" => new.snet = date(value)?,
                "snlt" => new.limit.snlt = date(value)?,
                "fnet" => new.limit.fnet = finish(value)?,
                "fnlt" => new.limit.fnlt = finish(value)?,
                "hostStart" | "hostFinish" => {
                    let kind = usize::from(&**name == "hostFinish");
                    let at = [Session::read_date, Session::read_finish][kind](self, value);
                    let text = value.as_str().unwrap_or_default().to_owned();
                    new.host.push((kind, text, at.ok_or_else(bad)?));
                }
                "name" => {
                    let text = value.as_str().ok_or_else(bad)?.trim();
                    let rename = Edit::Rename {
                        task,
                        name: text.to_owned(),
                    };
                    rename.check_typed().map_err(refused)?;
                    text.clone_into(&mut new.name);
                }
                "progress" => {
                    let Value::Int(n) = value else {
                        return Err(bad());
                    };
                    let whole = u8::try_from(*n).ok().filter(|&n| n <= 100);
                    percent = Some(whole.ok_or_else(|| refused(EditError::BadPercent))?);
                }
                _ => return Err(UpdateError::new("notUpdatable", Some(at), Some(name))),
            }
        }
        if let Some(percent) = percent {
            old.progress = Some(Progress {
                start: current.actual_start,
                finish: current.actual_finish,
                remaining: current.remaining,
            });
            new.percent = Some(percent);
        }
        Ok((new, old))
    }

    fn put(&mut self, task: TaskId, values: &Values) -> Result<(), EditError> {
        self.schedule.set_duration(task, values.duration)?;
        self.schedule.set_snet(task, values.snet)?;
        self.schedule.set_limit(task, values.limit)?;
        if let Some(progress) = values.progress {
            self.schedule.set_remaining(task, progress.remaining)?;
            self.schedule.set_actual_start(task, progress.start)?;
            self.schedule.set_actual_finish(task, progress.finish)?;
        }
        values.name.clone_into(&mut self.meta[task.0 as usize].name);
        Ok(())
    }

    /// Put every task in `done` back, newest first.
    fn put_back(&mut self, done: &[(TaskId, Values)]) {
        for (task, old) in done.iter().rev() {
            let _ = self.put(*task, old);
        }
    }
}
