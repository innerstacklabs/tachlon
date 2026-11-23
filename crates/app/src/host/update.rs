//! A host's new values for many tasks, by key, as one recompute (decision 0019, rule 13). It is not an
//! edit: no gate, no undo step, no outbox commit, and `revision` stays; `stamp` rises, so the table
//! redraws. Every record is read before a value is written, and a refusal puts every value back.

mod record;

use super::{Plan, differs::is_host, read::read_duration};
use crate::{
    Session,
    document::json_read::{Value, parse_host},
    edit::{Edit, EditError},
    history::{Source, Step},
};
use engine::{Instant, Limit, Minutes, Pin, TaskId};
use std::collections::HashSet;

/// How many tasks an update changed and how many times it solved: twice if it mixes progress with timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updated {
    pub tasks: usize,
    pub solves: usize,
    /// One JSON object (`record`, `key`, `member`, `host`, `chart`) per `host*` member the chart computed otherwise.
    pub differs: Vec<String>,
}

/// Why an update was refused. Nothing changed. `kind` is `badJson`, `noKey`, `unknownKey` (a
/// deleted task's key too), `duplicateKey`, `notUpdatable` (a member an update does not set, or a summary),
/// `badValue`, or an [`EditError::kind`]: `busy` and `schedule`, and for a `name` or a `progress` the
/// reasons a typed rename or percent gets (`emptyName`, `invalidName`, `badPercent`, `actualAfterDataDate`), and for an
/// `actualStart` or `actualFinish` `finishBeforeStart` and `actualAfterDataDate`.
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

/// What an update sets on one task. A record with none of `progress`, `actualStart`, `actualFinish` and
/// `remaining` leaves `progress` and `percent` empty, and the task's actual dates and remaining work are not written.
#[derive(Clone)]
pub(super) struct Values {
    duration: Minutes,
    snet: Option<Instant>,
    limit: Limit,
    name: String,
    progress: Option<Progress>,
    percent: Option<u8>,
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

fn pin_of(pin: Option<Pin>, member: &str) -> Option<Instant> {
    let fin = |p: &Pin| matches!(p, Pin::Finish(_)) == (member == "mfo");
    pin.filter(fin).map(|(Pin::Start(at) | Pin::Finish(at))| at)
}

impl Values {
    /// The steps that put `old` back where `self` is, one for each of the name, duration, start floor and limits
    /// that differ, in the order a call sets them: an undo runs them newest first.
    pub(super) fn steps_back(&self, old: &Values, task: TaskId) -> Vec<Step> {
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
    /// with a duration, start, snet or limit in any record solves twice, and any other call once. `actualStart` and `actualFinish` take a date or `null` (a date alone is read as plain data reads it: the first working minute of the day, and the end of its last) and `remaining` a whole number of working minutes or `null`; a record that gives `remaining` with `progress` (beside an actual date, `progress` is read and sets nothing: the dates stand, as a mount of the same task reads them), an actual finish before the start, a finish or remaining (unless `unstartedRemaining` is on) with no actual start, or an actual after the data date is refused. A member a
    /// record leaves out keeps its value. `hostStart`, `hostFinish`, `hostLateStart`, `hostLateFinish`, `hostFloat`, `hostFreeFloat` and `hostCritical`, what the host computed, set nothing and are named in `differs` when the chart computed otherwise (a day alone compares the day, a finish's day being its last minute's; a float is in working days of the task's calendar, and may have a fraction). `mso` and `mfo`, the pins, are read and never set: one that equals what the task's pin holds (`null` with no such pin) changes nothing, and any other is `notUpdatable`, as is every other member, `alap` included.
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
    /// [`Session::host_edit`]. With `undoable` a record whose Undo the engine would refuse is refused
    /// (`hasActuals`, `notAllowed`), and a call that changes no value answers without a solve.
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
        let root = parse_host(json).map_err(|_| bad_json())?;
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
        if undoable {
            self.undo_refusal(&planned)?;
            if (planned.iter()).all(|(task, (new, old))| new.steps_back(old, *task).is_empty()) {
                let differs = self.differences(records, &planned)?;
                let tasks = planned.len();
                return Ok((
                    Updated {
                        tasks,
                        solves: 0,
                        differs,
                    },
                    planned,
                ));
            }
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
            if let Err(error) = self.put(*task, new) {
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
        let differs = self.differences(records, &planned).inspect_err(|_| {
            self.put_back(&done);
            let _ = self.solve_full(None, |_| {});
        })?;
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
            differs,
        };
        Ok((updated, planned))
    }

    /// The first record of `planned` whose Undo would be refused: one that sets progress (it writes actual dates)
    /// or changes a task that has them (`hasActuals`), or whose put-back the profile check of Undo's gate refuses
    /// (`notAllowed`, what it names in `member`): the same function answers, so no copy of it can fall behind.
    fn undo_refusal(&self, planned: &[Written]) -> Result<(), UpdateError> {
        for (at, (task, (new, old))) in planned.iter().enumerate() {
            let t = &self.schedule.tasks()[task.0 as usize];
            let held = t.actual_start.or(t.actual_finish).is_some();
            let back: Vec<_> = (new.steps_back(old, *task).into_iter())
                .map(|step| step.edit)
                .collect();
            if new.percent.is_some() || new.progress.is_some() || held && !back.is_empty() {
                return Err(UpdateError::new("hasActuals", Some(at), None));
            }
            if let Err(error) = self.refuse_unallowed(&Plan::Group(&back, Source::Host)) {
                let what = match &error {
                    EditError::NotAllowed(what) => Some(*what),
                    EditError::NotStored(what) => Some(what.as_str()),
                    _ => None,
                };
                return Err(UpdateError::new(error.kind(), Some(at), what));
            }
        }
        Ok(())
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
        let mut old = self.values_of(task);
        let members: &[_] = match record {
            Value::Object(members)
                if !self.outline.as_ref().is_some_and(|o| o.is_summary(task)) =>
            {
                members
            }
            _ => return Err(UpdateError::new("notUpdatable", Some(at), Some("key"))),
        };
        let held = Progress::of(current);
        let mut new = old.clone();
        let (mut percent, mut actuals) = (None, None::<Progress>);
        let mut said: Vec<&str> = Vec::new();
        for (name, value) in members {
            let bad = || UpdateError::new("badValue", Some(at), Some(name));
            let refused = |error: EditError| UpdateError::new(error.kind(), Some(at), Some(name));
            let date = |value: &Value| match value {
                Value::Null => Ok(None),
                _ => self.read_date(value).map(Some).ok_or_else(bad),
            };
            let limit = |value: &Value, finish| match value {
                Value::Null => Ok(None),
                _ => (self.read_limit(task, value, finish))
                    .map(Some)
                    .ok_or_else(bad),
            };
            match &**name {
                "key" => {}
                "duration" => new.duration = read_duration(value).ok_or_else(bad)?,
                "start" if record.get("snet").is_some() => {
                    date(value)?;
                }
                "start" => new.snet = date(value)?,
                "snet" => new.snet = limit(value, false)?,
                "snlt" => new.limit.snlt = limit(value, false)?,
                "fnet" => new.limit.fnet = limit(value, true)?,
                "fnlt" => new.limit.fnlt = limit(value, true)?,
                "mso" | "mfo"
                    if limit(value, &**name == "mfo")? == pin_of(current.limit.pin, name) => {}
                host if is_host(host) => {
                    self.read_host(host, value).ok_or_else(bad)?;
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
                "actualStart" | "actualFinish" | "remaining" => {
                    let given = actuals.get_or_insert(held);
                    self.read_actual(task, name, value, given).ok_or_else(bad)?;
                    said.push(name);
                }
                "progress" => {
                    let Value::Int(n) = value else {
                        return Err(bad());
                    };
                    let whole = u8::try_from(*n).ok().filter(|&n| n <= 100);
                    percent = Some(whole.ok_or_else(|| refused(EditError::BadPercent))?);
                    said.push(name);
                }
                _ => return Err(UpdateError::new("notUpdatable", Some(at), Some(name))),
            }
        }
        if percent.is_some() || actuals.is_some() {
            old.progress = Some(held);
        }
        new.percent = percent;
        if let Some(given) = actuals {
            self.check_actuals(at, task, given, &said)?;
            new.percent = None; // the given dates stand, and a percent beside them sets nothing
            new.progress = Some(given);
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
