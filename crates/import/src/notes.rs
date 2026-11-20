use std::collections::{HashMap, HashSet};

use engine::Instant;

use crate::ImportedTask;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    File,
    Project,
    Calendar,
    Activity,
    Relationship,
}

impl Section {
    fn index(self) -> usize {
        match self {
            Self::File => 0,
            Self::Project => 1,
            Self::Calendar => 2,
            Self::Activity => 3,
            Self::Relationship => 4,
        }
    }
}

#[derive(Debug)]
struct Entry {
    kind: String,
    count: usize,
    ids: Vec<String>,
}

/// The unsupported-feature lines of one import, in a stable order.
///
/// XER features are counted by kind inside a [`Section`] ([`Notes::add`], [`Notes::add_many`]).
/// MSPDI features are whole sentences that are kept once, in the order met ([`Notes::line`]).
/// [`Notes::finish`] lists the counted kinds by section, then the sentences.
pub(crate) struct Notes {
    sections: [Vec<Entry>; 5],
    /// For each section, the position in `sections` of each kind.
    positions: [HashMap<String, usize>; 5],
    lines: Vec<String>,
    seen: HashSet<String>,
    /// The kind and id of each count made by [`Notes::add_once`].
    once: HashSet<(String, String)>,
}

impl Notes {
    pub(crate) fn new() -> Notes {
        Notes {
            sections: std::array::from_fn(|_| Vec::new()),
            positions: std::array::from_fn(|_| HashMap::new()),
            lines: Vec::new(),
            seen: HashSet::new(),
            once: HashSet::new(),
        }
    }

    /// Record `message` word for word. A message already recorded is dropped.
    pub(crate) fn line(&mut self, message: String) {
        if !self.seen.contains(&message) {
            self.seen.insert(message.clone());
            self.lines.push(message);
        }
    }

    /// How many sentences are recorded.
    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }

    /// Put each sentence of `inserts` at its position, counted in the sentences recorded now, as
    /// if it had been recorded there. `inserts` is in order of position; a sentence already
    /// recorded is dropped.
    pub(crate) fn splice(&mut self, inserts: Vec<(usize, String)>) {
        let mut inserts = inserts.into_iter().peekable();
        let old = std::mem::take(&mut self.lines);
        let mut lines = Vec::with_capacity(old.len() + inserts.len());
        for (index, line) in old.into_iter().enumerate() {
            while let Some((_, message)) = inserts.next_if(|(at, _)| *at <= index) {
                if self.seen.insert(message.clone()) {
                    lines.push(message);
                }
            }
            lines.push(line);
        }
        for (_, message) in inserts {
            if self.seen.insert(message.clone()) {
                lines.push(message);
            }
        }
        self.lines = lines;
    }

    /// The entry for `kind` in `section`, created empty after the existing ones when new.
    fn entry(&mut self, section: Section, kind: &str) -> Option<&mut Entry> {
        self.entry_at(section.index(), kind)
    }

    fn entry_at(&mut self, index: usize, kind: &str) -> Option<&mut Entry> {
        let entries = self.sections.get_mut(index)?;
        let positions = self.positions.get_mut(index)?;
        let position = if let Some(position) = positions.get(kind) {
            *position
        } else {
            positions.insert(kind.to_owned(), entries.len());
            entries.push(Entry {
                kind: kind.to_owned(),
                count: 0,
                ids: Vec::new(),
            });
            entries.len().saturating_sub(1)
        };
        entries.get_mut(position)
    }

    /// Record everything `other` holds as if it had been recorded here next, in its order.
    pub(crate) fn absorb(&mut self, other: Notes) {
        for (index, entries) in other.sections.into_iter().enumerate() {
            for theirs in entries {
                if let Some(entry) = self.entry_at(index, &theirs.kind) {
                    entry.count = entry.count.saturating_add(theirs.count);
                    for id in theirs.ids {
                        if entry.ids.len() < 5 {
                            entry.ids.push(id);
                        }
                    }
                }
            }
        }
        for line in other.lines {
            self.line(line);
        }
    }

    pub(crate) fn add(&mut self, section: Section, kind: &str, id: Option<&str>) {
        let Some(entry) = self.entry(section, kind) else {
            return;
        };
        entry.count = entry.count.saturating_add(1);
        if let Some(id) = id
            && entry.ids.len() < 5
        {
            entry.ids.push(id.to_owned());
        }
    }

    /// [`Notes::add`] for a task, counted once per `kind` however many of its values are noted.
    pub(crate) fn add_once(&mut self, section: Section, kind: &str, id: &str) {
        if self.once.insert((kind.to_owned(), id.to_owned())) {
            self.add(section, kind, Some(id));
        }
    }

    pub(crate) fn add_many(&mut self, section: Section, kind: &str, count: usize) {
        if count == 0 {
            return;
        }
        if let Some(entry) = self.entry(section, kind) {
            entry.count = entry.count.saturating_add(count);
        }
    }

    pub(crate) fn finish(self) -> Vec<String> {
        self.sections
            .into_iter()
            .flatten()
            .map(|entry| {
                if entry.ids.is_empty() {
                    format!("{}: {}", entry.kind, entry.count)
                } else {
                    format!(
                        "{}: {} (first: {})",
                        entry.kind,
                        entry.count,
                        entry.ids.join(", ")
                    )
                }
            })
            .chain(self.lines)
            .collect()
    }
}

/// One counted note for the tasks with an actual start or finish after the data date, with the
/// first five task ids. The dates are kept as the file states them; an actual later than the data
/// date moves every date after it.
pub(crate) fn note_actuals_after_data_date(
    tasks: &[ImportedTask],
    data_date: Option<Instant>,
    notes: &mut Notes,
) {
    let Some(data_date) = data_date else {
        return;
    };
    for task in tasks {
        if task
            .actual_start
            .into_iter()
            .chain(task.actual_finish)
            .any(|actual| actual > data_date)
        {
            let label = task.code.clone().unwrap_or_else(|| task.uid.to_string());
            notes.add(
                Section::Activity,
                "tasks with an actual date after the data date, kept as the file states them",
                Some(&label),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Notes, Section};

    #[test]
    fn formats_in_section_and_first_seen_order() {
        let mut notes = Notes::new();
        notes.add(Section::Activity, "later", Some("A2"));
        notes.add(Section::File, "first", None);
        notes.add_many(Section::Activity, "plain", 3);
        notes.add(Section::Activity, "later", Some("A3"));
        assert_eq!(
            notes.finish(),
            ["first: 1", "later: 2 (first: A2, A3)", "plain: 3"]
        );
    }

    #[test]
    fn remembers_only_the_first_five_ids() {
        let mut notes = Notes::new();
        for id in ["1", "2", "3", "4", "5", "6"] {
            notes.add(Section::Relationship, "links", Some(id));
        }
        assert_eq!(notes.finish(), ["links: 6 (first: 1, 2, 3, 4, 5)"]);
    }

    #[test]
    fn keeps_each_line_once_in_the_order_met() {
        let mut notes = Notes::new();
        notes.line("task 2: deadline not supported".to_owned());
        notes.line("task 1: deadline not supported".to_owned());
        notes.line("task 2: deadline not supported".to_owned());
        notes.line("task 1: deadline not supported".to_owned());
        assert_eq!(
            notes.finish(),
            [
                "task 2: deadline not supported",
                "task 1: deadline not supported"
            ]
        );
    }

    #[test]
    fn lists_lines_after_the_counted_kinds() {
        let mut notes = Notes::new();
        notes.line("a line".to_owned());
        notes.add(Section::File, "a kind", None);
        assert_eq!(notes.finish(), ["a kind: 1", "a line"]);
    }

    #[test]
    fn absorbed_notes_follow_what_is_already_recorded() {
        let mut notes = Notes::new();
        notes.add(Section::Calendar, "first", Some("1"));
        notes.line("a line".to_owned());
        let mut later = Notes::new();
        later.add(Section::Calendar, "second", None);
        later.add(Section::Calendar, "first", Some("2"));
        later.add(Section::File, "file", None);
        later.line("a line".to_owned());
        later.line("another line".to_owned());
        notes.absorb(later);
        assert_eq!(
            notes.finish(),
            [
                "file: 1",
                "first: 2 (first: 1, 2)",
                "second: 1",
                "a line",
                "another line"
            ]
        );
    }
}
