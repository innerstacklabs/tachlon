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
    /// Freed by a re-key: it stays in the probe chain, no lookup finds it, and `add` of the same text takes it again.
    dead: bool,
}

/// The map's hasher: `hash` already mixes the low bits the table reads; the multiply fills the high bits, which a `u32` leaves zero.
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
    let [_, _, _, _, a, b, c, d] = (mixed ^ mixed >> 32).wrapping_mul(MIX).to_le_bytes();
    u32::from_le_bytes([a, b, c, d])
}

impl Keys {
    /// Take `key` for slot `slot` of `kind`. A key already taken gives the slot that holds it.
    pub(crate) fn add(&mut self, kind: Kind, slot: usize, key: &str) -> Result<(), usize> {
        let mut at = hash(key);
        let id = loop {
            match self.heads[kind as usize].entry(at) {
                Occupied(held) => {
                    let id = *held.get();
                    let entry = &mut self.entries[id];
                    if self.text[entry.range.clone()] != *key {
                        at = at.wrapping_add(1);
                    } else if entry.dead {
                        (entry.dead, entry.slot) = (false, slot);
                        break id;
                    } else {
                        return Err(entry.slot);
                    }
                }
                Vacant(free) => {
                    let id = self.entries.len();
                    free.insert(id);
                    let start = self.text.len();
                    self.text.push_str(key);
                    let range = start..self.text.len();
                    self.entries.push(Entry {
                        slot,
                        range,
                        dead: false,
                    });
                    break id;
                }
            }
        };
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
            if !entry.dead && self.text[entry.range.clone()] == *key {
                return Some(entry);
            }
            at = at.wrapping_add(1);
        }
    }

    /// Free `key` of `kind`: mark its entry dead, so `find` no longer answers it and `add` can take the text again.
    fn release(&mut self, kind: Kind, key: &str) {
        let mut at = hash(key);
        while let Some(&id) = self.heads[kind as usize].get(&at) {
            let entry = &mut self.entries[id];
            if !entry.dead && self.text[entry.range.clone()] == *key {
                entry.dead = true;
                if let (Kind::Link, Some(by_slot)) = (kind, self.links.get_mut(entry.slot)) {
                    *by_slot = None;
                }
                return;
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
        self.give_key(target, key, false)
    }

    /// Give the task or link in `target` the key `key` the server answered for a row the chart held under another
    /// key (a restored row, decision 0022 rule 5): [`Session::assign_key`] without the `HasKey` refusal, and the old
    /// key is free afterwards. No edit and no history entry.
    ///
    /// # Errors
    ///
    /// As [`Session::assign_key`], except [`EditError::HasKey`]. Nothing changes on a refusal.
    pub fn rekey(&mut self, target: KeyTarget, key: &str) -> Result<(), EditError> {
        self.give_key(target, key, true)
    }

    fn give_key(&mut self, target: KeyTarget, key: &str, replace: bool) -> Result<(), EditError> {
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
            Some(_) if !replace => return Err(EditError::HasKey),
            _ => {}
        }
        if self.host.keys.find(kind, key).is_some() {
            return Err(EditError::KeyTaken);
        }
        if let Some(old) = held.map(str::to_owned) {
            self.host.keys.release(kind, &old);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(keys: impl Iterator<Item = String>, mask: u64) -> usize {
        let at = |key: String| {
            let mut spread = Spread::default();
            spread.write_u32(hash(&key));
            spread.finish() & mask
        };
        keys.map(at).collect::<std::collections::HashSet<_>>().len()
    }

    #[test]
    fn a_freed_key_keeps_the_probe_chain_and_its_text_can_be_taken_again() {
        let mut seen = HashMap::new();
        let (a, b) = (0u32..)
            .find_map(|n| {
                let key = format!("k{n}");
                seen.insert(hash(&key), key.clone())
                    .map(|first| (first, key))
            })
            .unwrap();
        let mut keys = Keys::default();
        keys.add(Kind::Task, 0, &a).unwrap();
        keys.add(Kind::Task, 1, &b).unwrap();
        for n in 2..2000 {
            keys.add(Kind::Task, n, &format!("other-{n}")).unwrap();
        }
        keys.release(Kind::Task, &a);
        assert_eq!(keys.task(&a), None);
        assert_eq!(keys.task(&b), Some(TaskId(1)), "found past the freed entry");
        assert!((2..2000u32).all(|n| keys.task(&format!("other-{n}")) == Some(TaskId(n))));
        keys.add(Kind::Task, 7000, &a).unwrap();
        assert_eq!(keys.task(&a), Some(TaskId(7000)));
    }

    #[test]
    fn keys_that_differ_in_the_last_bytes_spread_over_the_table() {
        let tail = |n: u32, w: u32| -> String {
            (0..w)
                .rev()
                .map(|i| char::from_u32(48 + (n >> (6 * i) & 63)).unwrap())
                .collect()
        };
        let two = (0..4096).map(|n| format!("abcdef{}", tail(n, 2)));
        let three = (0..50_000).map(|n| format!("project-task-{}", tail(n, 3)));
        assert!(positions(two, 4095) >= 2000);
        assert!(positions(three, 65_535) >= 30_000);
        assert!(positions((0..50_000).map(|n| format!("T{n}")), 65_535) >= 30_000);
    }
}
