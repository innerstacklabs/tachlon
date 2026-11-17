//! Comparing two files (gs#703): [`Session::compare_file`] lines up each task of a second parsed
//! file, last month's, with the task of the open session that is the same task, and returns what the
//! second file says about it as a [`Baseline`]. [`Session::set_baseline_from_file`] installs it, as
//! one undo step.
//!
//! The same task is the one with the same ID: the activity ID for an XER file, the task UID for an
//! MS Project file. Position decides nothing, so a file that lists its tasks in another order, or has
//! gained and lost some, still matches task by task.

use crate::Session;
use crate::baseline::{Baseline, BaselineTask};
use crate::dates::Origin;
use crate::edit::{Edit, EditError, format_link_text};
use crate::json::quote;
use crate::meta::TaskMeta;
use crate::texts::{
    COMPARE_NO_SHARED_ID, COMPARE_ONLY_HERE, COMPARE_ONLY_OLDER, Texts, Worded, english_display,
};
use engine::{DAY, LinkType, Schedule, Solution, TaskId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The most tasks or links one list of a [`CompareReport`] names (`file_only`, `here_only`,
/// `file_only_links`, `here_only_links`); a longer list ends with `and N more`.
pub const FILE_ONLY_MAX: usize = 200;

/// Why [`Session::compare_file`] returned no baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    /// No task has the same ID in both files: they are of different formats, or are different
    /// projects.
    NoSharedId,
    /// [`Session::set_baseline_from_file`]'s edit was refused.
    Edit(EditError),
}

impl From<EditError> for CompareError {
    fn from(error: EditError) -> CompareError {
        CompareError::Edit(error)
    }
}

impl Worded for CompareError {
    fn text(&self, texts: &Texts) -> String {
        match self {
            CompareError::NoSharedId => texts.say(&COMPARE_NO_SHARED_ID, &[]),
            CompareError::Edit(error) => error.text(texts),
        }
    }
}
english_display!(CompareError);

impl std::error::Error for CompareError {}

/// What setting a file as the comparison matched, as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompareReport {
    /// Tasks here that took an entry from the file.
    pub matched: u32,
    /// Tasks only the file has: the summary counts them as removed.
    pub only_in_file: u32,
    /// Live tasks only this session has: the summary counts them as added.
    pub only_here: u32,
    /// The tasks only the file has, one line each in file order (`A1020 Pour slab`: the ID, a
    /// space, the name), the first [`FILE_ONLY_MAX`] and then `and N more`.
    pub file_only: Vec<String>,
    /// The tasks only this session has (new work), named as `file_only` names them, in this
    /// session's order.
    pub here_only: Vec<String>,
    /// The links only the file has, one line each in the file's order (`A1010 → A1020 (FS)`: the
    /// two IDs and the type), the first [`FILE_ONLY_MAX`] and then `and N more`. A link is the
    /// same in both files when its predecessor ID, successor ID and type are.
    pub file_only_links: Vec<String>,
    /// The links only this session has, named as `file_only_links` names them, in this session's
    /// order.
    pub here_only_links: Vec<String>,
}

impl CompareReport {
    /// `{"matched":n,"only_in_file":n,"only_here":n,"lines":[...],"file_only":[...],"here_only":[...],
    /// "file_only_links":[...],"here_only_links":[...]}`, `lines` holding a sentence for each of
    /// the two unmatched counts that is not 0.
    #[must_use]
    pub fn json(&self) -> String {
        self.json_in(&Texts::default())
    }

    /// [`CompareReport::json`] with the `lines` sentences in `texts`' words.
    fn json_in(&self, texts: &Texts) -> String {
        let lines: Vec<String> = [
            (&COMPARE_ONLY_OLDER, self.only_in_file),
            (&COMPARE_ONLY_HERE, self.only_here),
        ]
        .into_iter()
        .filter(|&(_, count)| count > 0)
        .map(|(text, count)| quote(&texts.say(text, &[("count", &count.to_string())])))
        .collect();
        let quoted = |names: &[String]| {
            let names: Vec<String> = names.iter().map(|line| quote(line)).collect();
            names.join(",")
        };
        format!(
            r#"{{"matched":{},"only_in_file":{},"only_here":{},"lines":[{}],"file_only":[{}],"here_only":[{}],"file_only_links":[{}],"here_only_links":[{}]}}"#,
            self.matched,
            self.only_in_file,
            self.only_here,
            lines.join(","),
            quoted(&self.file_only),
            quoted(&self.here_only),
            quoted(&self.file_only_links),
            quoted(&self.here_only_links)
        )
    }
}

/// How a file names its tasks. An imported XER activity carries both an activity ID and a UID; an
/// imported MS Project task has a UID only. A task inserted here has an activity ID and no UID, so
/// it never says what the file was.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Xer,
    Mspdi,
}

/// The ID that names a task in a file of a [`Format`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Id<'a> {
    Activity(&'a str),
    Uid(u32),
}

impl std::fmt::Display for Id<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Activity(code) => f.write_str(code),
            Id::Uid(uid) => write!(f, "{uid}"),
        }
    }
}

impl Format {
    fn of(meta: &[TaskMeta]) -> Option<Format> {
        if meta
            .iter()
            .any(|task| task.uid.is_some() && task.code.is_some())
        {
            Some(Format::Xer)
        } else if meta.iter().any(|task| task.uid.is_some()) {
            Some(Format::Mspdi)
        } else {
            None
        }
    }

    /// The format both task metas have, if they have the same one.
    fn shared(meta: &[TaskMeta], other: &[TaskMeta]) -> Option<Format> {
        Format::of(meta).filter(|&format| Some(format) == Format::of(other))
    }

    fn id(self, task: &TaskMeta) -> Option<Id<'_>> {
        let uid = task.uid?;
        match self {
            Format::Xer => task.code.as_deref().map(Id::Activity),
            Format::Mspdi => Some(Id::Uid(uid)),
        }
    }
}

/// The live tasks of `from`, `(ID, name)` in its order, whose ID no live task of `other` has (there
/// they are absent or deleted, so no slot holds them). An ID `from` repeats counts once, at its
/// first task. Empty when the two share no format. Each is a schedule and its task meta.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
)]
fn only_in<'a>(
    (schedule, meta): (&Schedule, &'a [TaskMeta]),
    (other_schedule, other_meta): (&Schedule, &[TaskMeta]),
) -> Vec<(Id<'a>, &'a str)> {
    let Some(format) = Format::shared(meta, other_meta) else {
        return Vec::new();
    };
    let there: HashSet<Id> = other_meta
        .iter()
        .enumerate()
        .filter(|&(index, _)| !other_schedule.is_deleted(TaskId(index as u32)))
        .filter_map(|(_, task)| format.id(task))
        .collect();
    let mut seen = HashSet::new();
    meta.iter()
        .enumerate()
        .filter(|&(index, _)| !schedule.is_deleted(TaskId(index as u32)))
        .filter_map(|(_, task)| Some((format.id(task)?, task.name.as_str())))
        .filter(|(id, _)| !there.contains(id) && seen.insert(*id))
        .collect()
}

/// A link as a file names it: the predecessor's ID, the successor's ID and the type.
type Link<'a> = (Id<'a>, Id<'a>, LinkType);

/// The live links of a file in slot order, those whose two ends both have an ID in `format`.
fn live_links<'a>(format: Format, schedule: &Schedule, meta: &'a [TaskMeta]) -> Vec<Link<'a>> {
    let id = |task: TaskId| format.id(meta.get(task.0 as usize)?);
    schedule
        .live_dependencies()
        .filter_map(|(_, dep)| Some((id(dep.pred)?, id(dep.succ)?, dep.kind)))
        .collect()
}

/// The live links of `from` that no live link of `other` matches by (predecessor ID, successor ID,
/// type), in slot order: a link `from` repeats counts once, at its first slot. Empty when the two
/// share no format.
fn links_only_in<'a>(
    (schedule, meta): (&Schedule, &'a [TaskMeta]),
    (other_schedule, other_meta): (&Schedule, &[TaskMeta]),
) -> Vec<Link<'a>> {
    let Some(format) = Format::shared(meta, other_meta) else {
        return Vec::new();
    };
    let there: HashSet<Link> = live_links(format, other_schedule, other_meta)
        .into_iter()
        .collect();
    let mut seen = HashSet::new();
    live_links(format, schedule, meta)
        .into_iter()
        .filter(|link| !there.contains(link) && seen.insert(*link))
        .collect()
}

/// A list as a report names it: one line each, the first [`FILE_ONLY_MAX`] and then `and N more`.
fn capped<T>(only: &[T], texts: &Texts, line: impl Fn(&T) -> String) -> Vec<String> {
    let mut lines: Vec<String> = only.iter().take(FILE_ONLY_MAX).map(line).collect();
    if only.len() > FILE_ONLY_MAX {
        let more = (only.len() - FILE_ONLY_MAX).to_string();
        lines.push(texts.say(&crate::texts::COMPARE_MORE, &[("count", &more)]));
    }
    lines
}

/// A task list as a report names it: `ID name`.
fn named(only: &[(Id<'_>, &str)], texts: &Texts) -> Vec<String> {
    capped(only, texts, |(id, name)| format!("{id} {name}"))
}

/// A link list as a report names it: `pred → succ (type)`.
fn linked(only: &[Link<'_>], texts: &Texts) -> Vec<String> {
    capped(only, texts, |&(pred, succ, kind)| {
        // A zero lag prints the type alone.
        format!("{pred} → {succ} ({})", format_link_text(kind, 0, 0))
    })
}

impl Session {
    /// The second file as a baseline of the open session: for each live task here, the early dates
    /// and float the second file's task with the same ID has. `schedule`, `solution`, `meta` and
    /// `origin` are the second file's, as a load returns them. Its instants count from its own origin,
    /// so they move by `(its origin - this origin) * DAY`, the same date on this session's axis.
    /// A task with no partner in the other file, and a deleted one, has no entry; the baseline
    /// holds no links ([`Session::set_baseline_from_file`] lists them). A task only the second file has is counted in the summary's `removed`.
    ///
    /// # Errors
    /// [`CompareError::NoSharedId`] when the two files are of different formats, or no task has the
    /// same ID in both.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    pub fn compare_file(
        &self,
        schedule: &Schedule,
        solution: &Solution,
        meta: &[TaskMeta],
        origin: Origin,
    ) -> Result<Baseline, CompareError> {
        let format = Format::of(&self.meta)
            .filter(|&here| Some(here) == Format::of(meta))
            .ok_or(CompareError::NoSharedId)?;
        let mut there = HashMap::new();
        for (index, task) in meta.iter().enumerate() {
            if let Some(id) = format.id(task)
                && !schedule.is_deleted(TaskId(index as u32))
            {
                there.entry(id).or_insert(index);
            }
        }
        let shift = (origin.day() - self.origin.day()) * DAY;
        let tasks: Vec<Option<BaselineTask>> = self
            .meta
            .iter()
            .enumerate()
            .map(|(index, task)| {
                if self.schedule.is_deleted(TaskId(index as u32)) {
                    return None;
                }
                let dates = solution.dates.get(*there.get(&format.id(task)?)?)?;
                Some(BaselineTask {
                    es: dates.es + shift,
                    ef: dates.ef + shift,
                    float: Some(dates.float),
                })
            })
            .collect();
        if tasks.iter().all(Option::is_none) {
            return Err(CompareError::NoSharedId);
        }
        let unmatched = self.only_in_file(schedule, meta).len();
        Ok(Baseline::from_parts(tasks, solution.project_finish + shift)
            .with_unmatched(u32::try_from(unmatched).unwrap_or(u32::MAX)))
    }

    /// The tasks only the second file has, `(ID, name)` in the file's order: see [`only_in`].
    fn only_in_file<'a>(
        &self,
        schedule: &Schedule,
        meta: &'a [TaskMeta],
    ) -> Vec<(Id<'a>, &'a str)> {
        only_in((schedule, meta), (&self.schedule, self.meta.as_slice()))
    }

    /// The tasks only this session has, `(ID, name)` in its order: see [`only_in`].
    fn only_here(&self, schedule: &Schedule, meta: &[TaskMeta]) -> Vec<(Id<'_>, &str)> {
        only_in((&self.schedule, self.meta.as_slice()), (schedule, meta))
    }

    /// The links only the second file has, in the file's order: see [`links_only_in`].
    fn links_only_in_file<'a>(&self, schedule: &Schedule, meta: &'a [TaskMeta]) -> Vec<Link<'a>> {
        links_only_in((schedule, meta), (&self.schedule, self.meta.as_slice()))
    }

    /// The links only this session has, in its order: see [`links_only_in`].
    fn links_only_here(&self, schedule: &Schedule, meta: &[TaskMeta]) -> Vec<Link<'_>> {
        links_only_in((&self.schedule, self.meta.as_slice()), (schedule, meta))
    }

    /// Make the second file the comparison: [`Session::compare_file`]'s baseline replaces the one
    /// in force through [`Edit::SetBaseline`], so it is one undo step and undo hands back the old
    /// baseline, or none. The arguments are as for `compare_file`. Nothing changes on `Err`.
    ///
    /// # Errors
    /// What `compare_file` returns, or the edit's refusal.
    pub fn set_baseline_from_file(
        &mut self,
        schedule: &Schedule,
        solution: &Solution,
        meta: &[TaskMeta],
        origin: Origin,
    ) -> Result<CompareReport, CompareError> {
        let baseline = self.compare_file(schedule, solution, meta, origin)?;
        let matched = u32::try_from(baseline.len()).unwrap_or(u32::MAX);
        let file_only = named(&self.only_in_file(schedule, meta), &self.texts);
        let here_only = named(&self.only_here(schedule, meta), &self.texts);
        let file_only_links = linked(&self.links_only_in_file(schedule, meta), &self.texts);
        let here_only_links = linked(&self.links_only_here(schedule, meta), &self.texts);
        self.edit(Edit::SetBaseline {
            baseline: Some(Arc::new(baseline)),
        })?;
        // No entry is kept for a task deleted here, so the summary's removed is the file's alone.
        let summary = self.baseline_diff().unwrap_or_default();
        Ok(CompareReport {
            matched,
            only_in_file: summary.removed,
            only_here: summary.added,
            file_only,
            here_only,
            file_only_links,
            here_only_links,
        })
    }

    /// `report` as [`CompareReport::json`] writes it, its `lines` in the words of this chart's
    /// `set_locale`.
    #[must_use]
    pub fn compare_json(&self, report: &CompareReport) -> String {
        report.json_in(&self.texts)
    }
}
