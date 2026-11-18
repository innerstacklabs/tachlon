//! The schedule a document's calendars, tasks and links build, in the order [`super::read`] has
//! always read them: calendars, the lag calendar, tasks, the parent check, links. Each task and
//! each link is built as the JSON reader hands it over, so no tree of them is held; what the
//! reader did not stream (an array that came too soon, or that is not an array) is read from the
//! top-level members once the parse is done. The first refusal is kept and nothing is read after
//! it, so the error `read` returns is the one an all-at-once read would have named first.

use super::super::fields::{list, within};
use super::super::json_read::{Member, Value};
use super::floor::{Saved, install, read_floor};
use super::items::{add_link, build_calendar, lag_calendar, read_task};
use super::{DocumentError, Field, invalid};
use crate::host::{Keys, Kind};
use crate::meta::TaskMeta;
use engine::Schedule;

/// The next step of the build; each runs once, in this order.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Step {
    Calendars,
    LagCalendar,
    Tasks,
    Links,
    Done,
}

/// The task, counted from one, after which room for the rest is reserved: by then the file has
/// shown how many bytes a task takes.
const RESERVE_AT: usize = 4096;

pub(super) struct Build {
    pub(super) schedule: Schedule,
    pub(super) meta: Vec<TaskMeta>,
    pub(super) progress: Vec<u8>,
    pub(super) keys: Keys,
    floors: Vec<Saved>,
    /// The length of the text, in bytes.
    len: usize,
    next: Step,
    /// The tasks, and the links, arrived through [`Build::feed`], not from the members.
    fed: [bool; 2],
    error: Option<DocumentError>,
}

fn member<'a>(members: &'a [Member<'a>], key: &str) -> Field<&'a Value<'a>> {
    let found = members.iter().find(|(k, _)| k == key);
    found
        .map(|(_, v)| v)
        .ok_or_else(|| format!("{key}: missing key"))
}

impl Build {
    pub(super) fn new(len: usize) -> Self {
        Build {
            schedule: Schedule::new(),
            meta: Vec::new(),
            progress: Vec::new(),
            keys: Keys::default(),
            floors: Vec::new(),
            len,
            next: Step::Calendars,
            fed: [false; 2],
            error: None,
        }
    }

    /// Take element `index` of the array under `key`, which the reader streams; it ends at byte
    /// `end` of the text.
    pub(super) fn feed(
        &mut self,
        members: &[Member],
        key: &str,
        index: usize,
        end: usize,
        item: &Value,
    ) {
        let links = key == "links";
        self.until(if links { Step::Links } else { Step::Tasks }, members);
        self.fed[usize::from(links)] = true;
        self.keep(|b| {
            if links {
                b.link(index, item)
            } else {
                b.task(index, item)?;
                if index + 1 == RESERVE_AT {
                    b.reserve_rest(end);
                }
                Ok(())
            }
        });
    }

    /// Reserve the room for the tasks still to come, from the length of the text and the bytes
    /// the first `RESERVE_AT` took. The text also holds links, so this is a little high;
    /// [`Build::end_tasks`] gives back what is not used. A reservation that cannot be made is
    /// skipped: the lists grow as they would have.
    fn reserve_rest(&mut self, end: usize) {
        let rest = (self.len / (end / RESERVE_AT).max(1)).saturating_sub(RESERVE_AT);
        self.schedule.reserve_tasks(rest);
        let _ = self.meta.try_reserve_exact(rest);
        let _ = self.progress.try_reserve_exact(rest);
        let _ = self.floors.try_reserve_exact(rest);
    }

    /// Everything not read yet, from the top-level `root`; the first refusal, if there was one.
    pub(super) fn finish(mut self, root: &Value) -> Result<Self, DocumentError> {
        let members: &[Member] = match root {
            Value::Object(members) => members,
            _ => &[],
        };
        self.until(Step::Done, members);
        self.keys.shrink();
        self.error.take().map_or(Ok(self), Err)
    }

    fn keep(&mut self, step: impl FnOnce(&mut Self) -> Result<(), DocumentError>) {
        if self.error.is_none() {
            self.error = step(self).err();
        }
    }

    fn until(&mut self, to: Step, members: &[Member]) {
        while self.next < to {
            let step = self.next;
            self.next = match step {
                Step::Calendars => Step::LagCalendar,
                Step::LagCalendar => Step::Tasks,
                Step::Tasks => Step::Links,
                Step::Links | Step::Done => Step::Done,
            };
            self.keep(|b| b.run(step, members));
        }
    }

    fn run(&mut self, step: Step, members: &[Member]) -> Result<(), DocumentError> {
        match step {
            Step::Calendars => {
                let calendars = member(members, "calendars").map_err(invalid)?;
                for (i, calendar) in list(calendars, "calendars")
                    .map_err(invalid)?
                    .iter()
                    .enumerate()
                {
                    let built = build_calendar(calendar)
                        .map_err(|e| within(&format!("calendars[{i}]"), &e));
                    let (calendar, key) = built.map_err(invalid)?;
                    self.schedule.add_calendar(calendar);
                    self.take_key(Kind::Calendar, i, key.as_deref())?;
                }
            }
            Step::LagCalendar => {
                let lag = member(members, "lag_calendar").map_err(invalid)?;
                let bad = |e: String| invalid(format!("lag_calendar: {e}"));
                let lag = lag_calendar(lag).map_err(bad)?;
                self.schedule
                    .set_lag_calendar(lag)
                    .map_err(|e| bad(e.to_string()))?;
            }
            Step::Tasks => {
                if !self.fed[0] {
                    let tasks = member(members, "tasks").map_err(invalid)?;
                    for (i, item) in list(tasks, "tasks").map_err(invalid)?.iter().enumerate() {
                        self.task(i, item)?;
                    }
                }
                self.end_tasks()?;
            }
            Step::Links => {
                if !self.fed[1] {
                    let links = member(members, "links").map_err(invalid)?;
                    for (i, link) in list(links, "links").map_err(invalid)?.iter().enumerate() {
                        self.link(i, link)?;
                    }
                }
            }
            Step::Done => {}
        }
        Ok(())
    }

    fn task(&mut self, i: usize, item: &Value) -> Result<(), DocumentError> {
        let at = |e: String| invalid(within(&format!("tasks[{i}]"), &e));
        let (task, task_meta, percent) = read_task(item).map_err(at)?;
        self.schedule
            .add_task(task)
            .map_err(|e| invalid(format!("tasks[{i}]: {e}")))?;
        self.take_key(Kind::Task, i, task_meta.key.as_deref())?;
        self.meta.push(task_meta);
        self.progress.push(percent);
        self.floors.push(read_floor(item).map_err(at)?);
        Ok(())
    }

    fn link(&mut self, i: usize, item: &Value) -> Result<(), DocumentError> {
        let key = add_link(&mut self.schedule, item)
            .map_err(|e| invalid(within(&format!("links[{i}]"), &e)))?;
        self.take_key(Kind::Link, i, key)
    }

    /// Record the key of record `i` of its array, when it has one; a key already taken is refused.
    fn take_key(&mut self, kind: Kind, i: usize, key: Option<&str>) -> Result<(), DocumentError> {
        let Some(key) = key else { return Ok(()) };
        let array = match kind {
            Kind::Task => "tasks",
            Kind::Link => "links",
            Kind::Calendar => "calendars",
        };
        self.keys
            .add(kind, i, key)
            .map_err(|first| DocumentError::DuplicateKey {
                key: key.to_string(),
                first: format!("{array}[{first}]"),
                again: format!("{array}[{i}]"),
            })
    }

    /// A parent names a task, so it is checked against the count once every task is read.
    fn end_tasks(&mut self) -> Result<(), DocumentError> {
        let count = self.meta.len();
        let bad = (self.meta.iter().enumerate())
            .find_map(|(i, m)| m.parent.filter(|p| p.0 as usize >= count).map(|p| (i, p.0)));
        if let Some((i, p)) = bad {
            return Err(invalid(within(
                &format!("tasks[{i}]"),
                &format!("parent: no task {p}"),
            )));
        }
        install(&mut self.meta, &self.floors, &self.schedule);
        // The lists were sized from a guess or grew with the stream; the count is known now, so
        // return the spare room.
        self.schedule.shrink_tasks();
        self.meta.shrink_to_fit();
        self.progress.shrink_to_fit();
        self.floors = Vec::new();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::json_read::parse_streaming;

    #[test]
    fn room_for_the_rest_is_reserved_at_the_4096th_task_and_given_back_at_the_end() {
        let task = r#"{"duration":60,"calendar":0,"snet":null,"actual_start":null,"actual_finish":null,"remaining":null,"name":"t","parent":null,"outline_level":0,"code":null,"wbs_code":null,"activity_codes":[],"progress":0}"#;
        let link = r#"{"pred":0,"succ":1,"kind":"FS","lag":0,"lag_calendar":null}"#;
        let (tasks, links) = (3 * RESERVE_AT, RESERVE_AT);
        let week = ["[[540,1020]]"; 7].join(",");
        let text = format!(
            r#"{{"calendars":[{{"week":[{week}],"exceptions":[]}}],"lag_calendar":"successor","tasks":[{}],"links":[{}]}}"#,
            vec![task; tasks].join(","),
            vec![link; links].join(",")
        );
        let mut build = Build::new(text.len());
        let mut reserved = [0; 3];
        let root = parse_streaming(&text, &mut |members, key, index, end, item| {
            build.feed(members, key, index, end, &item);
            if key == "tasks" && index + 1 == RESERVE_AT {
                reserved = [
                    build.meta.capacity(),
                    build.progress.capacity(),
                    build.floors.capacity(),
                ];
            }
        });
        let done = build.finish(&root.unwrap()).unwrap();
        assert!(reserved.iter().all(|c| *c > tasks), "{reserved:?}");
        assert_eq!(
            (done.meta.capacity(), done.progress.capacity()),
            (tasks, tasks)
        );
    }
}
