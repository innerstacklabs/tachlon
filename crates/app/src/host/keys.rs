//! The keys a host gave its tasks, links and calendars (decision 0016): the one map over them.
//! A task's key is stored on its `TaskMeta`; a link's and a calendar's are stored here, by slot.

use crate::Session;
use crate::edit::EditError;
use engine::{CalendarId, DepId, TaskId};
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;

/// The three kinds of record a key names. One text may be a key of each kind.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    /// By kind, the entry of a key under its hash. A key whose hash another text holds goes under the next
    /// number up, so a lookup walks up from its hash until it finds the text or a free number.
    heads: [HashMap<u32, usize, BuildHasherDefault<Spread>>; 3],
    links: Vec<Option<usize>>,
    calendars: Vec<Option<usize>>,
}

const MIX: u64 = 0x517c_c1b7_2722_0a95;

struct Entry {
    slot: usize,
    range: Range<usize>,
}

/// The map's hasher: a key's hash is mixed already, so this only spreads it into the bits the table reads.
#[derive(Default)]
struct Spread(u64);

impl Hasher for Spread {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(MIX);
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = u64::from(n).wrapping_mul(MIX);
    }
}

/// The text's hash, taken a word at a time. A collision costs one more text compare, never a wrong answer.
fn hash(key: &str) -> u32 {
    let mut mixed = 0u64;
    for chunk in key.as_bytes().chunks(8) {
        let mut word = [0; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        mixed = (mixed.rotate_left(5) ^ u64::from_le_bytes(word)).wrapping_mul(MIX);
    }
    let [_, _, _, _, a, b, c, d] = mixed.to_le_bytes();
    u32::from_le_bytes([a, b, c, d])
}

impl Keys {
    /// Take `key` for slot `slot` of `kind`. A key already taken gives the slot that holds it.
    pub(crate) fn add(&mut self, kind: Kind, slot: usize, key: &str) -> Result<(), usize> {
        let id = self.entries.len();
        let mut at = hash(key);
        loop {
            match self.heads[kind as usize].entry(at) {
                Occupied(held) if self.text[self.entries[*held.get()].range.clone()] == *key => {
                    return Err(self.entries[*held.get()].slot);
                }
                Occupied(_) => at = at.wrapping_add(1),
                Vacant(free) => {
                    free.insert(id);
                    break;
                }
            }
        }
        let start = self.text.len();
        self.text.push_str(key);
        let range = start..self.text.len();
        self.entries.push(Entry { slot, range });
        let by_slot = match kind {
            Kind::Task => return Ok(()),
            Kind::Link => &mut self.links,
            Kind::Calendar => &mut self.calendars,
        };
        by_slot.resize(by_slot.len().max(slot + 1), None);
        by_slot[slot] = Some(id);
        Ok(())
    }

    /// Give back the room the lists grew past the keys read.
    pub(crate) fn shrink(&mut self) {
        self.text.shrink_to_fit();
        self.entries.shrink_to_fit();
        self.links.shrink_to_fit();
        self.heads.iter_mut().for_each(HashMap::shrink_to_fit);
    }

    fn find(&self, kind: Kind, key: &str) -> Option<&Entry> {
        let mut at = hash(key);
        loop {
            let entry = &self.entries[*self.heads[kind as usize].get(&at)?];
            if self.text[entry.range.clone()] == *key {
                return Some(entry);
            }
            at = at.wrapping_add(1);
        }
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
