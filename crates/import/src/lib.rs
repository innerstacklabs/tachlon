//! Import project schedules from Microsoft Project XML and Primavera P6 XER files.

#![cfg_attr(not(test), warn(clippy::too_many_lines))]

mod activity_id;
mod bytes;
mod calendar_rule;
mod calendars;
mod costs;
mod deadline;
mod dom;
mod export;
mod fields;
mod floor;
mod mspdi;
mod notes;
mod periods;
mod range;
mod resources;
mod tasks;
mod units;
mod xer_activities;
mod xer_activity_order;
mod xer_calendars;
mod xer_codes;
mod xer_import;
mod xer_links;
mod xer_progress;
mod xer_wbs;
mod xer_wbs_code;

use std::fmt;

use engine::{CalendarError, Instant, Minutes, Schedule, ScheduleError, TaskId};

use notes::Notes;

pub use activity_id::next_activity_id;
pub use bytes::{is_xer, parse_bytes};
pub use export::{Exported, XerLayout, XerOptions, export_mspdi, export_xer};
pub use xer_activity_order::natural;
pub use xer_import::{ProjectSelection, parse_xer};

/// A schedule and its metadata imported from an external project file.
#[derive(Debug, Clone)]
pub struct Imported {
    /// The imported engine schedule.
    pub schedule: Schedule,
    /// Task metadata in the same order as [`Schedule::tasks`].
    pub tasks: Vec<ImportedTask>,
    /// Input features that were ignored or approximated, in document order.
    pub unsupported: Vec<String>,
    /// The data date, as an instant from the schedule origin: an XER project's `last_recalc_date`
    /// (the latest of them with several selected projects) or the MSPDI `StatusDate`.
    /// `None` when the file states none, or when it is unreadable or falls before the origin.
    pub data_date: Option<Instant>,
    /// The calendar date of instant 0, as days since 1970-01-01: the Monday on or before the
    /// plan start (XER `plan_start_date`, the earliest of the selected projects) or the MSPDI
    /// `StartDate`. A file with no start gets Monday 2000-01-03, and says so in `unsupported`.
    pub origin_day: i64,
    /// The instant every task is floored at: the MSPDI `StartDate` as an instant from the origin
    /// (the origin is the Monday on or before it, so this is not always 0). `None` when the file
    /// states no start, and always for XER, whose projects each have their own. The MS Project
    /// writer needs it to write the same `StartDate` back.
    pub project_start: Option<Instant>,
}

/// Metadata retained for one imported task.
#[derive(Debug, Clone, Default)]
pub struct ImportedTask {
    /// The source UID. XER WBS summaries and activities have separate ID spaces,
    /// so their UIDs may overlap.
    pub uid: u32,
    /// The P6 activity code, or `None` for MSPDI tasks and XER summaries.
    pub code: Option<String>,
    /// The task name, or an empty string when it was absent.
    pub name: String,
    /// The task's outline level.
    pub outline_level: u8,
    /// The nearest preceding task with a smaller outline level.
    pub parent: Option<TaskId>,
    /// The start the file states, as an instant from the schedule origin (the engine does not use
    /// it): P6's early start for an XER activity, the task's own `Start` for an MS Project task.
    /// `None` for a summary in either format, an absent date, or a date the origin arithmetic
    /// cannot represent.
    pub file_start: Option<Instant>,
    /// The finish the file states, as `file_start`: P6's early finish for an XER activity, the
    /// task's own `Finish` for an MS Project task.
    pub file_finish: Option<Instant>,
    /// Whether the task is in progress: an XER activity with status `TK_Active`, or an MS Project
    /// task with an `ActualStart` and no `ActualFinish`. `false` for a summary in either format.
    pub in_progress: bool,
    /// The actual start the file states, as an instant from the schedule origin (negative when it
    /// falls before it). XER: `act_start_date` of an Active or Complete activity; `None` for any
    /// other activity, a summary and an absent date. MS Project: the task's
    /// `ActualStart`, with the same exceptions.
    pub actual_start: Option<Instant>,
    /// The actual finish the file states, as `actual_start`: XER `act_end_date` of a Complete
    /// activity, MS Project `ActualFinish` of a task with an `ActualStart`.
    pub actual_finish: Option<Instant>,
    /// The remaining duration in working minutes: XER `remain_drtn_hr_cnt` of an Active or
    /// Complete activity, MS Project `RemainingDuration`
    /// of a task with an `ActualStart`. `None` when the file states none, and for everything else.
    pub remaining: Option<Minutes>,
    /// The task's saved baseline start, as an instant from the schedule origin: the `Start` of
    /// the MSPDI `Baseline` numbered 0, negative when it falls before the origin. `None` when the
    /// file states no baseline start, and always for XER tasks.
    pub baseline_start: Option<Instant>,
    /// The task's saved baseline finish, as `baseline_start`. A baseline can state either date
    /// without the other.
    pub baseline_finish: Option<Instant>,
    /// Percent complete, 0 to 100, for display only: XER `phys_complete_pct` or `complete_pct` by
    /// the declared type, MSPDI `PercentComplete`. `None` when the file states none, and for XER
    /// summaries.
    pub progress: Option<u8>,
    /// The P6 activity codes assigned to the task, in the order the file lists them. Empty for
    /// MSPDI tasks, XER summaries and activities with no activity code row.
    pub activity_codes: Vec<ActivityCode>,
    /// The task's WBS code. XER: the `wbs_short_name` of each WBS node from the project root down to
    /// the activity's own node (or the summary itself), joined with a dot. MSPDI: the `WBS` text. `None`
    /// when the file states none.
    pub wbs_code: Option<String>,
    /// The instant the importer floors the task at: its project's start, from the schedule
    /// origin. `None` when the project states no start, and for an XER summary, which is not
    /// floored. A task constraint counts only when it is strictly above this; one at or below it
    /// is overridden.
    pub floor: Option<Instant>,
    /// The task's deadline, as an instant from the schedule origin (negative when it falls before
    /// it). MS Project: the `Deadline`. `None` when the file states none or an unreadable one, and
    /// always for XER tasks. No date follows from it yet.
    pub deadline: Option<Instant>,
    /// The names of the people or crews assigned to the task, once each, in the order the file
    /// lists its assignments: the `Name` of each MS Project `Resource` that an `Assignment` of
    /// the task points at. A resource with no name, or one the file does not list, is left out.
    /// Empty for XER tasks and for a task with no assignment. No date follows from it.
    pub resource_names: Vec<String>,
}

/// One activity code assigned to a task: the code type's name (`Discipline`) and the code's value
/// (`ELEC`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityCode {
    /// The activity code type name.
    pub code_type: String,
    /// The activity code's short name, or its full name when the file gives no short name.
    pub value: String,
}

/// An error that prevents an external project document from being imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The XML document is not well formed.
    Xml {
        /// A parser message, including the byte position when available.
        message: String,
    },
    /// The XER document could not be parsed or read.
    Xer {
        /// The XER reader's error message.
        message: String,
    },
    /// The XER document contains no projects.
    NoProjects,
    /// The XER document has several candidate projects and none was selected.
    ProjectChoice {
        /// Candidate projects in file order.
        projects: Vec<XerProject>,
    },
    /// A requested XER project does not occur in the file.
    UnknownProject {
        /// The requested project identifier.
        proj_id: u32,
    },
    /// An XER project, activity, or WBS identifier occurs more than once.
    DuplicateId {
        /// The XER table containing the duplicate.
        table: String,
        /// The repeated identifier.
        id: u32,
    },
    /// An XER WBS outline exceeds 250 levels.
    WbsTooDeep,
    /// A required element is absent.
    Missing {
        /// The local name of the missing element.
        element: String,
    },
    /// An element contains an invalid value.
    Invalid {
        /// The local name of the invalid element.
        element: String,
        /// The invalid text value.
        value: String,
    },
    /// Two imported tasks have the same UID.
    DuplicateUid {
        /// The repeated task UID.
        uid: u32,
    },
    /// A predecessor link refers to no imported task.
    UnknownPredecessor {
        /// The successor task UID.
        task_uid: u32,
        /// The missing predecessor task UID.
        predecessor_uid: u32,
    },
    /// An engine calendar rejected imported calendar data.
    Calendar {
        /// The calendar UID from the MSPDI document.
        uid: u32,
        /// The engine validation error.
        source: CalendarError,
    },
    /// The engine schedule rejected an imported task or dependency.
    Schedule(ScheduleError),
    /// The XML document contains more than 64 nested elements.
    TooDeep,
    /// The file is an MS Project binary file, not XML.
    NotXml,
    /// The file is not a schedule: a PDF, a ZIP archive, or XML that is not an MS Project file.
    NotASchedule {
        /// What the file is, as the words after "this is".
        what: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Xml { message } => write!(f, "XML error: {message}"),
            ImportError::Xer { message } => write!(f, "XER error: {message}"),
            ImportError::NoProjects => f.write_str("XER file contains no projects"),
            ImportError::ProjectChoice { projects } => {
                f.write_str("choose an XER project:")?;
                for project in projects.iter().take(20) {
                    write!(f, " id {}", project.proj_id)?;
                    if let Some(name) = &project.short_name {
                        write!(f, " {name:?}")?;
                    }
                    write!(f, " ({} activities);", project.activities)?;
                }
                if projects.len() > 20 {
                    write!(f, " and {} more", projects.len().saturating_sub(20))?;
                }
                Ok(())
            }
            ImportError::UnknownProject { proj_id } => {
                write!(f, "XER project id {proj_id} is not in the file")
            }
            ImportError::DuplicateId { table, id } => {
                write!(f, "duplicate {table} id {id}")
            }
            ImportError::WbsTooDeep => f.write_str("XER WBS outline exceeds 250 levels"),
            ImportError::Missing { element } => {
                write!(f, "missing required element {element}")
            }
            ImportError::Invalid { element, value } => {
                write!(f, "invalid {element} value {value:?}")
            }
            ImportError::DuplicateUid { uid } => write!(f, "duplicate task UID {uid}"),
            ImportError::UnknownPredecessor {
                task_uid,
                predecessor_uid,
            } => write!(
                f,
                "task {task_uid}: predecessor UID {predecessor_uid} does not name an imported task"
            ),
            ImportError::Calendar { uid, source } => {
                write!(f, "calendar {uid}: {source}")
            }
            ImportError::Schedule(source) => write!(f, "schedule: {source}"),
            ImportError::TooDeep => write!(f, "XML element nesting exceeds 64 levels"),
            ImportError::NotXml => f.write_str(
                "this is an MS Project binary file (.mpp); in MS Project choose File > Save As, pick the XML format, and open that file",
            ),
            ImportError::NotASchedule { what } => write!(
                f,
                "this is {what}, not a schedule; open a P6 XER file or an MS Project XML file"
            ),
        }
    }
}

/// The error for a file that is `what` and not a schedule.
pub(crate) fn not_a_schedule(what: &str) -> ImportError {
    ImportError::NotASchedule {
        what: what.to_owned(),
    }
}

/// A candidate project in an XER file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XerProject {
    /// The P6 project identifier.
    pub proj_id: u32,
    /// The P6 project short name.
    pub short_name: Option<String>,
    /// The number of activities belonging to the project.
    pub activities: usize,
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImportError::Calendar { source, .. } => Some(source),
            ImportError::Schedule(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ScheduleError> for ImportError {
    fn from(source: ScheduleError) -> Self {
        ImportError::Schedule(source)
    }
}

/// Parse a Microsoft Project XML document.
///
/// # Errors
///
/// Returns an [`ImportError`] when the XML is malformed or a value is out of range.
pub fn parse_mspdi(xml: &str) -> Result<Imported, ImportError> {
    mspdi::parse(xml)
}

/// The project's `StatusDate` as an instant from the origin Monday: the wall-clock date and time
/// as written, with any zone suffix ignored, so the day shown is the day in the file. A value
/// that is not a date, or a date before the origin, is dropped and noted, as the XER reader does.
fn status_date(root: &dom::Node, origin_day: i64, notes: &mut Notes) -> Option<i64> {
    let value = root.child_text("StatusDate")?;
    let instant = units::parse_datetime(value).and_then(|(day, minute)| {
        day.checked_sub(origin_day)
            .and_then(|days| days.checked_mul(1440))
            .and_then(|minutes| minutes.checked_add(i64::from(minute)))
    });
    if instant.is_none_or(|instant| instant < 0) {
        notes.line(format!(
            "Project: the status date {value:?} is not a date on or after the project start and was not read; dates of tasks that have started can differ from the file's."
        ));
        return None;
    }
    instant.and_then(|instant| range::data_date(instant, None, notes))
}

fn project_start(root: &dom::Node, notes: &mut Notes) -> Result<(i64, Option<i64>), ImportError> {
    let Some(value) = root.child_text("StartDate") else {
        notes.line(
            "Project: the file has no start date, so the schedule starts on Monday 2000-01-03 and every date follows from that."
                .to_owned(),
        );
        return Ok((units::days_from_civil(2000, 1, 3), None));
    };
    let (start_day, minute) = units::parse_datetime(value).ok_or_else(|| ImportError::Invalid {
        element: "StartDate".to_owned(),
        value: value.to_owned(),
    })?;
    let origin_day = start_day
        .checked_sub(i64::from(units::weekday_of(start_day)))
        .ok_or_else(|| ImportError::Invalid {
            element: "StartDate".to_owned(),
            value: value.to_owned(),
        })?;
    let start_instant = start_day
        .checked_sub(origin_day)
        .and_then(|days| days.checked_mul(1440))
        .and_then(|minutes| minutes.checked_add(i64::from(minute)))
        .ok_or_else(|| ImportError::Invalid {
            element: "StartDate".to_owned(),
            value: value.to_owned(),
        })?;
    Ok((origin_day, Some(start_instant)))
}
