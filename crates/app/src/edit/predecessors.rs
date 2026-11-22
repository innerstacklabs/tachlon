//! The tasks a task follows, typed as a list of their Activity IDs with an optional link type and
//! lag (gantt#2078): `3, 5SS+1d`.

use super::{Edit, EditError, LinkTextError, format_link_text, parse_link_text};
use crate::texts::{PREDECESSOR_ITSELF, PREDECESSOR_LINK, PREDECESSOR_NO_IDS, PREDECESSOR_TWICE};
use crate::texts::{PREDECESSOR_UNKNOWN, Texts, Worded};
use crate::{Session, history::Source};
use engine::{DepId, LinkType, Minutes, ScheduleError, TaskId};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// Why one item of a typed list was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredecessorFault {
    /// No live task has this Activity ID.
    Unknown,
    /// The item is the task the list is for.
    Itself,
    /// An earlier item names the same task.
    Twice,
    /// The schedule has no Activity IDs to type.
    NoIds,
    /// The text after the ID is not a link type and lag.
    Link(LinkTextError),
}

impl PredecessorFault {
    /// The sentence for `item` in the words of `texts`.
    pub(super) fn say(self, texts: &Texts, item: &str) -> String {
        match self {
            Self::Unknown => texts.say(&PREDECESSOR_UNKNOWN, &[("item", item)]),
            Self::Itself => texts.say(&PREDECESSOR_ITSELF, &[("item", item)]),
            Self::Twice => texts.say(&PREDECESSOR_TWICE, &[("item", item)]),
            Self::NoIds => texts.say(&PREDECESSOR_NO_IDS, &[]),
            Self::Link(error) => texts.say(
                &PREDECESSOR_LINK,
                &[("item", item), ("error", &error.text(texts))],
            ),
        }
    }
}

/// The live links into each task, as slots in order, with the `(revision, stamp)` they were read at.
type Kept = ((u32, u32), Vec<Vec<DepId>>);

/// The links kept by a [`Session`], so a table of rows reads them once.
#[derive(Debug, Default)]
pub(crate) struct Memo(pub(crate) Mutex<Option<Kept>>);

impl Session {
    /// The edits that make `task` follow exactly the tasks `text` lists (`3, 5SS+1d`: an Activity ID,
    /// then an optional link type and lag): removes first, then changes, then adds. A link already
    /// as typed gives none; empty `text` removes every link into `task`.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownTask`] for a task that is not live, [`EditError::Predecessor`] naming
    /// the item that is not a task, is `task` itself, repeats an item, or has bad link text.
    pub fn plan_predecessors_text(&self, task: TaskId, text: &str) -> Result<Vec<Edit>, EditError> {
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let current: Vec<(DepId, TaskId)> = self
            .schedule
            .live_dependencies()
            .filter(|(_, link)| link.succ == task)
            .map(|(dep, link)| (dep, link.pred))
            .collect();
        let items = text
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty());
        let ids = self.live_ids();
        let mut wanted: Vec<(TaskId, LinkType, Minutes)> = Vec::new();
        for item in items {
            let fault = |fault| EditError::Predecessor {
                item: item.to_string(),
                fault,
            };
            if ids.is_empty() {
                return Err(fault(PredecessorFault::NoIds));
            }
            let (pred, kind, lag) = self
                .resolve_item(&ids, task, &current, item)
                .map_err(fault)?;
            if pred == task {
                return Err(fault(PredecessorFault::Itself));
            }
            if wanted.iter().any(|&(held, ..)| held == pred) {
                return Err(fault(PredecessorFault::Twice));
            }
            wanted.push((pred, kind, lag));
        }
        let mut edits: Vec<Edit> = current
            .iter()
            .filter(|(_, pred)| wanted.iter().all(|&(held, ..)| held != *pred))
            .map(|&(dep, _)| Edit::RemoveLink { dep })
            .collect();
        for (pred, kind, lag) in wanted {
            match current.iter().find(|&&(_, held)| held == pred) {
                None => edits.push(Edit::AddLink {
                    pred,
                    succ: task,
                    kind,
                    lag,
                }),
                Some(&(dep, _)) => {
                    let held = self.schedule.dependencies()[dep.0 as usize];
                    if (held.kind, held.lag) != (kind, lag) {
                        edits.push(Edit::SetLink { dep, kind, lag });
                    }
                }
            }
        }
        Ok(edits)
    }

    /// Make `task` follow exactly the tasks `text` lists, as one undo step: the plan of
    /// [`Session::plan_predecessors_text`] run through [`Session::edit_group`]. Text that changes
    /// nothing records no step. A refusal puts back whatever already ran.
    ///
    /// # Errors
    /// The planning refusals, else the first edit refused: a summary end or a loop.
    pub fn set_predecessors_text(&mut self, task: TaskId, text: &str) -> Result<(), EditError> {
        let edits = self.plan_predecessors_text(task, text)?;
        if edits.is_empty() {
            return Ok(());
        }
        self.edit_group(edits, Source::Typed)
    }

    /// The tasks `task` follows as [`Session::set_predecessors_text`] reads them back (`3, 5 SS+1d`: a space
    /// keeps an ID that is another ID plus a type, `A5SS` beside `A5`, the task it is),
    /// in the order the links were made. `None` for a task that is not live or one that follows a
    /// task with no Activity ID. The links come from the engine's cached graph, which only a link
    /// change drops; without one they are kept per `(revision, stamp)`.
    #[must_use]
    pub fn predecessors_text(&self, task: TaskId) -> Option<String> {
        if !self.is_live(task) {
            return None;
        }
        let key = (self.revision(), self.stamp());
        let mut cached = self
            .predecessors
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let deps: Vec<DepId> = if let Some(deps) = self.schedule.cached_links_into(task) {
            deps.collect()
        } else {
            if cached.as_ref().is_none_or(|(kept, _)| *kept != key) {
                let mut into = vec![Vec::new(); self.schedule.tasks().len()];
                for (dep, link) in self.schedule.live_dependencies() {
                    into[link.succ.0 as usize].push(dep);
                }
                *cached = Some((key, into));
            }
            cached
                .as_ref()
                .map(|(_, into)| into[task.0 as usize].clone())?
        };
        let items: Option<Vec<String>> = deps
            .into_iter()
            .map(|dep| {
                let link = self.schedule.dependencies()[dep.0 as usize];
                let code = self.meta[link.pred.0 as usize].code.as_deref()?;
                let kind = match (link.kind, link.lag) {
                    (LinkType::FS, 0) => String::new(),
                    (kind, lag) => format!(
                        " {}",
                        format_link_text(kind, lag, self.lag_day_minutes(dep))
                    ),
                };
                Some(format!("{code}{kind}"))
            })
            .collect();
        items.map(|items| items.join(", "))
    }

    /// The live tasks with an Activity ID, by that ID in lower case; the first of two with one ID.
    fn live_ids(&self) -> HashMap<String, TaskId> {
        let mut ids = HashMap::new();
        for (index, meta) in self.meta.iter().enumerate() {
            let task = TaskId(u32::try_from(index).unwrap_or(u32::MAX));
            if let Some(code) = meta.code.as_deref().filter(|_| self.is_live(task)) {
                ids.entry(code.to_lowercase()).or_insert(task);
            }
        }
        ids
    }

    /// One item as a task, link type and lag: the longest start of `item` that is an Activity ID
    /// whose rest reads as link text, so `A5SS` is the task `A5SS` before `A5` with type `SS`.
    fn resolve_item(
        &self,
        ids: &HashMap<String, TaskId>,
        task: TaskId,
        current: &[(DepId, TaskId)],
        item: &str,
    ) -> Result<(TaskId, LinkType, Minutes), PredecessorFault> {
        let mut bad = None;
        for end in (1..=item.len())
            .rev()
            .filter(|&end| item.is_char_boundary(end))
        {
            let Some(&pred) = ids.get(&item[..end].to_lowercase()) else {
                continue;
            };
            let rest = item[end..].trim();
            if rest.is_empty() {
                return Ok((pred, LinkType::FS, 0));
            }
            let own = current
                .iter()
                .find(|&&(_, held)| held == pred)
                .and_then(|&(dep, _)| self.schedule.dependency_lag_calendar(dep));
            match parse_link_text(rest, self.pair_day_minutes(pred, task, own)) {
                Ok((kind, lag)) => return Ok((pred, kind, lag)),
                Err(error) => bad = bad.or(Some(error)),
            }
        }
        Err(bad.map_or(PredecessorFault::Unknown, PredecessorFault::Link))
    }
}
