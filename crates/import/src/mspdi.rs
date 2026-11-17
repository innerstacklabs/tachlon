//! The MS Project reader: the schedule is built while the XML is read, one `Calendar`, `Task` or
//! `Assignment` at a time (`dom::parse` hands each over as it closes), so the file is never a tree.
//!
//! A file lists the project's header, then its calendars, its tasks, and last its assignments,
//! and this reader relies on that. It raises the same first error and records the same notes in
//! the same order as a reader that holds the whole file, for the file in any order: what has to
//! wait for the end of the file waits (the links, the note for a task with an assignment, the
//! project-start floor), and a file whose header or calendars come after its tasks is read twice,
//! the second time once for each kind of element, in the order the header needs them, so
//! no more than one element is held at a time.

use std::collections::HashMap;

use engine::Schedule;

use crate::calendars::{CalendarSet, Calendars};
use crate::dom::{self, Item, Node};
use crate::fields::{optional_i64, optional_u32};
use crate::notes::{Notes, note_actuals_after_data_date};
use crate::resources;
use crate::tasks::Tasks;
use crate::{ImportError, Imported, not_a_schedule, project_start, status_date};

/// Parse a Microsoft Project XML document.
pub(crate) fn parse(xml: &str) -> Result<Imported, ImportError> {
    let mut build = Build::new();
    let root = dom::parse(xml, |root, item| build.item(root, item))?;
    if !build.out_of_order(&root) {
        return build.finish(&root);
    }
    // The file stated something after it was used. Read it again, one kind of element at a time
    // (calendars, assignments, tasks: the order the header needs them in) with `root`, the
    // whole file's, so no more than one element is ever held and the first pass's builder is gone.
    drop(build);
    let mut build = Build::new();
    for rank in 0..3 {
        dom::parse(xml, |_, item| {
            if item.rank() == rank {
                build.item(&root, item);
            }
        })?;
    }
    build.finish(&root)
}

impl Item<'_> {
    fn rank(&self) -> u8 {
        match self {
            Item::Calendar(_) => 0,
            Item::Assignment(_) => 1,
            Item::Task(_) => 2,
        }
    }
}

/// The three header values the calendars and dates are read with, to check the file did not
/// state any of them after it was used.
#[derive(PartialEq)]
struct Header {
    start: Option<String>,
    status: Option<String>,
    calendar: Option<String>,
}

impl Header {
    fn of(root: &Node) -> Header {
        let text = |name| root.child_text(name).map(str::to_owned);
        Header {
            start: text("StartDate"),
            status: text("StatusDate"),
            calendar: text("CalendarUID"),
        }
    }
}

/// The calendars: collected until the first task or assignment, then built.
enum Calendar {
    Reading(Option<CalendarSet>),
    Built(Option<Calendars>),
}

struct Build {
    schedule: Schedule,
    notes: Notes,
    /// The first error of the header, the calendars or the assignments. Each comes before every
    /// task's, so once one is set no further element is read.
    error: Option<ImportError>,
    task_error: Option<ImportError>,
    started: bool,
    header: Option<Header>,
    origin_day: i64,
    start_instant: Option<i64>,
    data_date: Option<i64>,
    calendars: Calendar,
    tasks: Tasks,
    /// Each assigned task's `TaskUID`, with the `ResourceUID` of its assignments in file order.
    assigned: HashMap<u32, Vec<i64>>,
    /// A calendar came after a task or an assignment.
    late_calendar: bool,
}

impl Build {
    fn new() -> Build {
        Build {
            schedule: Schedule::new(),
            notes: Notes::new(),
            error: None,
            task_error: None,
            started: false,
            header: None,
            origin_day: 0,
            start_instant: None,
            data_date: None,
            calendars: Calendar::Reading(None),
            tasks: Tasks::default(),
            assigned: HashMap::new(),
            late_calendar: false,
        }
    }

    fn fail(&mut self, error: ImportError) {
        self.error.get_or_insert(error);
    }

    /// Whether the file stated its start, status date or calendar after they were used, or a
    /// calendar after a task or an assignment.
    fn out_of_order(&self, root: &Node) -> bool {
        self.late_calendar
            || self
                .header
                .as_ref()
                .is_some_and(|used| *used != Header::of(root))
    }

    /// Read the project's own fields, once, with the root as it stands at the first element.
    fn start(&mut self, root: &Node) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        self.header = Some(Header::of(root));
        if root.name != "Project" {
            let name: String = root.name.chars().take(40).collect();
            return self.fail(not_a_schedule(&format!("an XML file whose root is {name}")));
        }
        match project_start(root, &mut self.notes) {
            Ok((origin_day, start_instant)) => {
                self.origin_day = origin_day;
                self.start_instant = start_instant;
                self.data_date = status_date(root, origin_day, &mut self.notes);
            }
            Err(error) => self.fail(error),
        }
    }

    fn item(&mut self, root: &Node, item: Item) {
        self.start(root);
        match item {
            Item::Calendar(node) => self.calendar(root, &node),
            Item::Assignment(node) => {
                self.close_calendars(root);
                if self.error.is_none() {
                    self.assignment(&node);
                }
            }
            Item::Task(node) => {
                self.close_calendars(root);
                if self.error.is_none() && self.task_error.is_none() {
                    self.task(&node);
                }
            }
        }
    }

    fn calendar(&mut self, root: &Node, node: &Node) {
        let Calendar::Reading(set) = &mut self.calendars else {
            self.late_calendar = true;
            return;
        };
        if self.error.is_some() {
            return;
        }
        let result = match set {
            Some(set) => Ok(set),
            None => CalendarSet::new(root).map(|new| set.insert(new)),
        };
        if let Err(error) = result.and_then(|set| set.add(node, self.origin_day, &mut self.notes)) {
            self.fail(error);
        }
    }

    /// Build the calendars once the file has moved on to the tasks.
    fn close_calendars(&mut self, root: &Node) {
        let Calendar::Reading(set) = &mut self.calendars else {
            return;
        };
        let set = set.take();
        self.calendars = Calendar::Built(None);
        if self.error.is_some() {
            return;
        }
        let built = match set {
            Some(set) => Ok(set),
            None => CalendarSet::new(root),
        }
        .and_then(|set| set.build(&mut self.schedule, &mut self.notes));
        match built {
            Ok(calendars) => self.calendars = Calendar::Built(Some(calendars)),
            Err(error) => self.fail(error),
        }
    }

    fn assignment(&mut self, node: &Node) {
        let read = || -> Result<Option<(u32, i64)>, ImportError> {
            let Some(task_uid) = optional_u32(node, "TaskUID")? else {
                return Ok(None);
            };
            let Some(resource_uid) = optional_i64(node, "ResourceUID")? else {
                return Ok(None);
            };
            Ok((resource_uid != -65535).then_some((task_uid, resource_uid)))
        };
        match read() {
            Ok(Some((task_uid, resource_uid))) => {
                let resources = self.assigned.entry(task_uid).or_default();
                if !resources.contains(&resource_uid) {
                    resources.push(resource_uid);
                }
            }
            Ok(None) => {}
            Err(error) => self.fail(error),
        }
    }

    fn task(&mut self, node: &Node) {
        let Calendar::Built(Some(calendars)) = &self.calendars else {
            return;
        };
        if let Err(error) = self.tasks.add(
            node,
            &mut self.schedule,
            calendars,
            self.origin_day,
            &mut self.notes,
        ) {
            self.task_error = Some(error);
        }
    }

    fn finish(mut self, root: &Node) -> Result<Imported, ImportError> {
        self.start(root);
        self.close_calendars(root);
        if let Some(error) = self.error.or(self.task_error) {
            return Err(error);
        }
        let mut tasks = self.tasks.finish(
            &mut self.schedule,
            &mut self.notes,
            &self.assigned,
            self.start_instant,
        )?;
        resources::name_tasks(&mut tasks, &self.assigned, root);
        note_actuals_after_data_date(&tasks, self.data_date, &mut self.notes);
        Ok(Imported {
            schedule: self.schedule,
            tasks,
            unsupported: self.notes.finish(),
            data_date: self.data_date,
            origin_day: self.origin_day,
            project_start: self.start_instant,
        })
    }
}
