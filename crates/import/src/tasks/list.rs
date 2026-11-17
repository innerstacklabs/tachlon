//! The tasks read so far: each `Task` is turned into its schedule task and its `ImportedTask` as
//! it closes, and what has to wait for the rest of the file waits here in a few words per task.

use std::collections::HashMap;

use engine::{Schedule, TaskId};

use super::links::{Link, add_links, read_links};
use super::{Read, read_task};
use crate::calendars::Calendars;
use crate::costs;
use crate::dom::Node;
use crate::fields::{flag, required_u32};
use crate::floor::{self, Floor};
use crate::notes::Notes;
use crate::resources;
use crate::{ImportError, ImportedTask};

/// The imported tasks in document order, and what the end of the file settles.
#[derive(Default)]
pub(crate) struct Tasks {
    imported: Vec<ImportedTask>,
    ids: Vec<TaskId>,
    floors: Vec<Floor>,
    /// Where in the notes each task's assignment note belongs, by position.
    assigned_at: Vec<usize>,
    by_uid: HashMap<u32, usize>,
    /// Not hidden by a later task of equal or lower level; levels rise.
    open: Vec<(TaskId, u8)>,
    links: Vec<Link>,
    /// How many tasks state a cost.
    costed: usize,
}

impl Tasks {
    /// Read one `Task` element and add it to `schedule`.
    pub(crate) fn add(
        &mut self,
        node: &Node,
        schedule: &mut Schedule,
        calendars: &Calendars,
        origin_day: i64,
        notes: &mut Notes,
    ) -> Result<(), ImportError> {
        if flag(node, "IsNull")? == Some(true) {
            return Ok(());
        }
        let uid = required_u32(node, "UID")?;
        if self.by_uid.contains_key(&uid) {
            return Err(ImportError::DuplicateUid { uid });
        }
        let Read {
            imported,
            floor,
            assigned_at,
        } = read_task(node, schedule, calendars, origin_day, &mut self.open, notes)?;
        self.costed += usize::from(costs::states_cost(node)?);
        let position = self.imported.len();
        read_links(node, position, &mut self.links);
        self.by_uid.insert(uid, position);
        self.open.push((floor.id, imported.outline_level));
        self.ids.push(floor.id);
        self.floors.push(floor);
        self.assigned_at.push(assigned_at);
        self.imported.push(imported);
        Ok(())
    }

    /// Settle what waited for the end of the file: the links, one note for the tasks named in
    /// `assigned` (the `TaskUID` of every assignment that has a resource, with its resources), one for the tasks that
    /// state a cost and the project-start floor. Returns the tasks.
    pub(crate) fn finish(
        mut self,
        schedule: &mut Schedule,
        notes: &mut Notes,
        assigned: &HashMap<u32, Vec<i64>>,
        start_instant: Option<i64>,
    ) -> Result<Vec<ImportedTask>, ImportError> {
        let notes_at_end = notes.len();
        let positions: Vec<usize> = self
            .imported
            .iter()
            .zip(&self.assigned_at)
            .filter(|(task, _)| assigned.contains_key(&task.uid))
            .map(|(_, at)| *at)
            .collect();
        if let Some(&first) = positions.first() {
            notes.splice(vec![(
                first.min(notes_at_end),
                resources::note(positions.len()),
            )]);
        }
        if let Some(note) = costs::note(self.costed) {
            notes.line(note);
        }
        let uids: Vec<u32> = self.imported.iter().map(|task| task.uid).collect();
        add_links(self.links, &self.ids, &uids, &self.by_uid, schedule, notes)?;
        floor::set_project_start(&mut self.floors, &mut self.imported, start_instant);
        floor::apply(&self.floors, schedule)?;
        Ok(self.imported)
    }
}
