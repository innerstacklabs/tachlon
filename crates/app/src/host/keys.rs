//! The keys a host gave its tasks, links and calendars (decision 0016): the one map over them.
//! A task's key is stored on its `TaskMeta`; a link's and a calendar's are stored here, by slot.

use crate::Session;
use crate::edit::EditError;
use engine::{CalendarId, DepId, TaskId};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

/// The three kinds of record a key names. One text may be a key of each kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Task,
    Link,
    Calendar,
}

/// Every key read at load, and the slot it names. A deleted record keeps its key, because an undo brings it back.
/// The text of every key is kept once, in `text`; an entry names a range of it, so a key costs no allocation.
#[derive(Default)]
pub struct Keys {
    text: String,
    entries: Vec<Entry>,
    /// The newest entry of the kind whose key has this hash; `Entry::next` leads to the older ones.
    heads: HashMap<(Kind, u64), usize>,
    links: Vec<Option<usize>>,
    calendars: Vec<Option<usize>>,
}

struct Entry {
    slot: usize,
    range: Range<usize>,
    next: Option<usize>,
}

fn hash(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

impl Keys {
    /// Take `key` for slot `slot` of `kind`. A key already taken gives the slot that holds it.
    pub(crate) fn add(&mut self, kind: Kind, slot: usize, key: &str) -> Result<(), usize> {
        if let Some(taken) = self.find(kind, key) {
            return Err(taken.slot);
        }
        let start = self.text.len();
        self.text.push_str(key);
        let id = self.entries.len();
        let next = self.heads.insert((kind, hash(key)), id);
        let range = start..self.text.len();
        self.entries.push(Entry { slot, range, next });
        let by_slot = match kind {
            Kind::Task => return Ok(()),
            Kind::Link => &mut self.links,
            Kind::Calendar => &mut self.calendars,
        };
        by_slot.resize(by_slot.len().max(slot + 1), None);
        by_slot[slot] = Some(id);
        Ok(())
    }

    fn find(&self, kind: Kind, key: &str) -> Option<&Entry> {
        let mut at = self.heads.get(&(kind, hash(key))).copied();
        while let Some(id) = at {
            let entry = &self.entries[id];
            if self.text[entry.range.clone()] == *key {
                return Some(entry);
            }
            at = entry.next;
        }
        None
    }

    fn spelling(&self, id: usize) -> &str {
        &self.text[self.entries[id].range.clone()]
    }

    fn slot(&self, kind: Kind, key: &str) -> Option<u32> {
        u32::try_from(self.find(kind, key)?.slot).ok()
    }

    /// The task that holds `key`, deleted or not.
    #[must_use]
    pub fn task(&self, key: &str) -> Option<TaskId> {
        self.slot(Kind::Task, key).map(TaskId)
    }

    /// The link that holds `key`, deleted or not.
    #[must_use]
    pub fn link(&self, key: &str) -> Option<DepId> {
        self.slot(Kind::Link, key).map(DepId)
    }

    /// The calendar that holds `key`.
    #[must_use]
    pub fn calendar(&self, key: &str) -> Option<CalendarId> {
        self.slot(Kind::Calendar, key).map(CalendarId)
    }

    /// The key of link `dep`, if it has one.
    #[must_use]
    pub fn link_key(&self, dep: DepId) -> Option<&str> {
        self.links.get(dep.0 as usize)?.map(|id| self.spelling(id))
    }

    /// The key of calendar `calendar`, if it has one.
    #[must_use]
    pub fn calendar_key(&self, calendar: CalendarId) -> Option<&str> {
        self.calendars
            .get(calendar.0 as usize)?
            .map(|id| self.spelling(id))
    }

    /// How many calendars hold a key.
    pub(crate) fn calendar_count(&self) -> usize {
        self.calendars.iter().flatten().count()
    }
}

impl Session {
    /// Put the keys a document held in force. None is an edit, so the history stays empty.
    pub fn with_host_keys(mut self, keys: Keys) -> Session {
        self.host.keys = keys;
        self
    }

    /// The keys of this session's links and calendars, and the key-to-record maps.
    #[must_use]
    pub fn host_keys(&self) -> &Keys {
        &self.host.keys
    }

    /// The task that holds `key`, unless it is deleted (an undo may bring it back, and the key map keeps it).
    #[must_use]
    pub fn task_for_key(&self, key: &str) -> Option<TaskId> {
        self.host
            .keys
            .task(key)
            .filter(|&task| !self.schedule.is_deleted(task))
    }
}

/// The record a key is handed to, by session slot.
#[derive(Clone, Copy)]
pub enum KeyTarget {
    Task(TaskId),
    Link(DepId),
}

impl Session {
    /// Give the task or link in `target` the key `key`, which the host chose after saving it. This is
    /// no edit: no history entry, no revision change. A deleted slot (an undo may bring it back) takes a
    /// key too, and the same key again changes nothing.
    ///
    /// # Errors
    ///
    /// [`EditError::EmptyKey`]; [`EditError::UnknownTask`] or [`EditError::UnknownLink`] for a slot never
    /// made; [`EditError::HasKey`] for a record that holds another key; [`EditError::KeyTaken`] when
    /// another record of that kind holds `key`. Nothing changes on a refusal.
    pub fn assign_key(&mut self, target: KeyTarget, key: &str) -> Result<(), EditError> {
        if key.is_empty() {
            return Err(EditError::EmptyKey);
        }
        let (kind, slot, held) = match target {
            KeyTarget::Task(task) => {
                let meta = self
                    .meta
                    .get(task.0 as usize)
                    .ok_or(EditError::UnknownTask(task))?;
                (Kind::Task, task.0 as usize, meta.key.as_deref())
            }
            KeyTarget::Link(dep) => {
                if dep.0 as usize >= self.schedule.dependencies().len() {
                    return Err(EditError::UnknownLink(dep));
                }
                (Kind::Link, dep.0 as usize, self.host.keys.link_key(dep))
            }
        };
        match held {
            Some(held) if held == key => return Ok(()),
            Some(_) => return Err(EditError::HasKey),
            None => {}
        }
        if self.host.keys.find(kind, key).is_some() {
            return Err(EditError::KeyTaken);
        }
        if kind == Kind::Task {
            self.meta[slot].key = Some(key.to_owned());
        }
        self.host
            .keys
            .add(kind, slot, key)
            .map_err(|_| EditError::KeyTaken)
    }
}
