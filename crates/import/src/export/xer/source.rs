//! The file the schedule was opened from, laid over the file the writer made (decision 0009,
//! item 2; XER Q5, Q6 and Q9).
//!
//! The writer numbers every id from 1. When a source is given this pass renumbers them to the
//! source's: a task by its activity ID, a project by its short name, a WBS node by its place under
//! the same parent and its name. A row with no match takes an id above the largest the source
//! holds, so an inserted row never reuses the id of one the schedule dropped. The `CALENDAR` rows
//! are the source's, as read, and a task takes its source row's calendar. The `ERMHDR` cells are
//! the source's, except the export time. The encoding and line ending are the source's. Source
//! rows nothing matched are not written, and are listed.

use std::collections::{HashMap, HashSet};

use xer::{Cell, Table, Writer};

use super::rows::cell_error;
use crate::ImportError;
use crate::fields::invalid;

/// A table's rows as decoded text, with its fields.
struct Sheet {
    fields: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Sheet {
    fn of(table: &Table<'_>) -> Sheet {
        let width = table.field_count();
        Sheet {
            fields: table.fields().map(std::borrow::Cow::into_owned).collect(),
            rows: table
                .rows()
                .iter()
                .map(|row| {
                    (0..width)
                        .map(|i| row.cell(i).unwrap_or_default().into_owned())
                        .collect()
                })
                .collect(),
        }
    }

    fn at(&self, field: &str) -> Option<usize> {
        self.fields.iter().position(|name| name == field)
    }

    /// The cell of `row` under `field`, empty when the table has no such field.
    fn get<'a>(&self, row: &'a [String], field: &str) -> &'a str {
        self.at(field)
            .and_then(|i| row.get(i))
            .map_or("", String::as_str)
    }

    /// The largest number any row holds under `field`.
    fn largest(&self, field: &str) -> u64 {
        self.rows
            .iter()
            .filter_map(|row| self.get(row, field).trim().parse::<u64>().ok())
            .max()
            .unwrap_or(0)
    }
}

/// Maps the ids of the written file to the ids the output uses, and hands out new ones.
struct Ids {
    next: u64,
    map: HashMap<String, String>,
}

impl Ids {
    fn new(source: &Sheet, field: &str) -> Ids {
        Ids {
            next: source.largest(field),
            map: HashMap::new(),
        }
    }

    fn keep(&mut self, from: &str, to: &str) {
        self.map.insert(from.to_owned(), to.to_owned());
    }

    fn fresh(&mut self, from: &str) -> String {
        self.next += 1;
        let id = self.next.to_string();
        self.map.insert(from.to_owned(), id.clone());
        id
    }

    fn get(&self, from: &str) -> String {
        self.map
            .get(from)
            .cloned()
            .unwrap_or_else(|| from.to_owned())
    }
}

fn sheet(file: &xer::Xer<'_>, name: &str) -> Option<Sheet> {
    file.table(name).map(Sheet::of)
}

fn empty() -> Sheet {
    Sheet {
        fields: Vec::new(),
        rows: Vec::new(),
    }
}

/// The index of the first row of `source` that `matches` and no earlier call claimed.
fn claim(
    source: &Sheet,
    claimed: &mut HashSet<usize>,
    matches: impl Fn(&[String]) -> bool,
) -> Option<usize> {
    let found = source
        .rows
        .iter()
        .enumerate()
        .find(|(i, row)| !claimed.contains(i) && matches(row))
        .map(|(i, _)| i);
    claimed.extend(found);
    found
}

/// List the rows of `source` that `claimed` does not hold: `template` with `@` for the `field`.
fn dropped(
    source: &Sheet,
    claimed: &HashSet<usize>,
    field: &str,
    template: &str,
    out: &mut Vec<String>,
) {
    for (i, row) in source.rows.iter().enumerate() {
        if !claimed.contains(&i) {
            out.push(template.replace('@', source.get(row, field)));
        }
    }
}

/// Lay `source` over `written`: the same file with the source's ids, `CALENDAR` rows and header.
/// What the source holds that the schedule no longer does goes to `left_out`.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pass that lays the schedule over each table; splitting it would only move the same locals into arguments"
)]
pub(super) fn apply(
    written: &[u8],
    source: &[u8],
    left_out: &mut Vec<String>,
) -> Result<Vec<u8>, ImportError> {
    let read = |bytes, what| xer::parse(bytes).map_err(|error| invalid(what, &error.to_string()));
    let (written, source) = (read(written, "export")?, read(source, "source")?);
    let table = |file: &xer::Xer<'_>, name| sheet(file, name).unwrap_or_else(empty);
    let (src_projects, src_wbs, src_tasks) = (
        table(&source, "PROJECT"),
        table(&source, "PROJWBS"),
        table(&source, "TASK"),
    );
    let replays = source.table("CALENDAR").is_some();
    let first_calendar = source
        .table("CALENDAR")
        .map(Sheet::of)
        .and_then(|sheet| {
            sheet
                .rows
                .first()
                .map(|row| sheet.get(row, "clndr_id").to_owned())
        })
        .unwrap_or_default();

    // Projects by short name.
    let gen_projects = table(&written, "PROJECT");
    let mut projects = Ids::new(&src_projects, "proj_id");
    let mut project_calendar: HashMap<String, String> = HashMap::new();
    let mut claimed = HashSet::new();
    for row in &gen_projects.rows {
        let (id, name) = (
            gen_projects.get(row, "proj_id"),
            gen_projects.get(row, "proj_short_name"),
        );
        // A project is its short name, or else the name of its root WBS node (the writer names a
        // project from its root summary).
        let by_name = |s: &[String]| src_projects.get(s, "proj_short_name") == name;
        let by_root = |s: &[String]| {
            src_wbs.rows.iter().any(|node| {
                src_wbs.get(node, "parent_wbs_id").is_empty()
                    && src_wbs.get(node, "wbs_name") == name
                    && src_wbs.get(node, "proj_id") == src_projects.get(s, "proj_id")
            })
        };
        let found = claim(&src_projects, &mut claimed, by_name)
            .or_else(|| claim(&src_projects, &mut claimed, by_root));
        if let Some(i) = found {
            let source_row = &src_projects.rows[i];
            projects.keep(id, src_projects.get(source_row, "proj_id"));
            project_calendar.insert(
                id.to_owned(),
                src_projects.get(source_row, "clndr_id").to_owned(),
            );
        } else {
            projects.fresh(id);
            project_calendar.insert(id.to_owned(), first_calendar.clone());
        }
    }
    dropped(
        &src_projects,
        &claimed,
        "proj_short_name",
        "source project @: the schedule has no such project, its row is not written",
        left_out,
    );

    // WBS nodes, parents first: a root by its project, any other by its parent and name.
    let gen_wbs = table(&written, "PROJWBS");
    let mut nodes = Ids::new(&src_wbs, "wbs_id");
    let mut claimed = HashSet::new();
    for row in &gen_wbs.rows {
        let id = gen_wbs.get(row, "wbs_id");
        let parent = gen_wbs.get(row, "parent_wbs_id");
        let project = projects.get(gen_wbs.get(row, "proj_id"));
        let name = gen_wbs.get(row, "wbs_name");
        let parent = (!parent.is_empty()).then(|| nodes.get(parent));
        let found = claim(&src_wbs, &mut claimed, |s| match &parent {
            None => {
                src_wbs.get(s, "parent_wbs_id").is_empty() && src_wbs.get(s, "proj_id") == project
            }
            Some(parent) => {
                src_wbs.get(s, "parent_wbs_id") == parent && src_wbs.get(s, "wbs_name") == name
            }
        });
        match found {
            Some(i) => nodes.keep(id, src_wbs.get(&src_wbs.rows[i], "wbs_id")),
            None => {
                nodes.fresh(id);
            }
        }
    }
    dropped(
        &src_wbs,
        &claimed,
        "wbs_name",
        "source WBS node @: the schedule has no such node, its row is not written",
        left_out,
    );

    // Tasks by activity ID.
    let gen_tasks = table(&written, "TASK");
    let mut tasks = Ids::new(&src_tasks, "task_id");
    let mut task_calendar: HashMap<String, String> = HashMap::new();
    let mut claimed = HashSet::new();
    for row in &gen_tasks.rows {
        let id = gen_tasks.get(row, "task_id");
        let code = gen_tasks.get(row, "task_code");
        let project = projects.get(gen_tasks.get(row, "proj_id"));
        let found = claim(&src_tasks, &mut claimed, |s| {
            src_tasks.get(s, "task_code") == code && src_tasks.get(s, "proj_id") == project
        });
        if let Some(i) = found {
            let source_row = &src_tasks.rows[i];
            tasks.keep(id, src_tasks.get(source_row, "task_id"));
            task_calendar.insert(
                id.to_owned(),
                src_tasks.get(source_row, "clndr_id").to_owned(),
            );
        } else {
            tasks.fresh(id);
            let project = gen_tasks.get(row, "proj_id");
            task_calendar.insert(
                id.to_owned(),
                project_calendar.get(project).cloned().unwrap_or_default(),
            );
        }
    }
    dropped(
        &src_tasks,
        &claimed,
        "task_code",
        "source task @: the schedule has no such activity, its row is not written",
        left_out,
    );

    // Links keep their source `task_pred_id`; a new one takes an id above the largest.
    let src_links = table(&source, "TASKPRED");
    let mut link_ids: HashMap<(String, String), String> = HashMap::new();
    for row in &src_links.rows {
        let key = (
            src_links.get(row, "task_id"),
            src_links.get(row, "pred_task_id"),
        );
        link_ids
            .entry((key.0.to_owned(), key.1.to_owned()))
            .or_insert_with(|| src_links.get(row, "task_pred_id").to_owned());
    }
    let mut next_link = src_links.largest("task_pred_id");

    // Write: the source's encoding, line ending and header, then the written tables.
    let mut writer = Writer::new(source.encoding(), source.line_ending());
    if source.byte_order_mark() {
        writer = writer.byte_order_mark();
    }
    let stamp = written.header().cell(1).unwrap_or_default();
    let header: Vec<Cell<'_>> = source
        .header()
        .raw_cells()
        .enumerate()
        .map(|(i, raw)| {
            if i == 1 {
                Cell::Text(&stamp)
            } else {
                Cell::Raw(raw)
            }
        })
        .collect();
    writer
        .header(&header)
        .map_err(|error| cell_error("ERMHDR", error))?;

    for out in written.tables() {
        let name = out.name().into_owned();
        let error = |error| cell_error(&name, error);
        let fields: Vec<String> = out.fields().map(std::borrow::Cow::into_owned).collect();
        if name == "CALENDAR"
            && let Some(replayed) = source.table("CALENDAR")
        {
            let replayed_fields: Vec<String> = replayed
                .fields()
                .map(std::borrow::Cow::into_owned)
                .collect();
            let replayed_fields: Vec<&str> = replayed_fields.iter().map(String::as_str).collect();
            writer.table(&name, &replayed_fields).map_err(error)?;
            for row in replayed.rows() {
                let cells: Vec<Cell<'_>> = row.raw_cells().map(Cell::Raw).collect();
                writer.row(&cells).map_err(error)?;
            }
            continue;
        }
        let names: Vec<&str> = fields.iter().map(String::as_str).collect();
        writer.table(&name, &names).map_err(error)?;
        let current = Sheet::of(out);
        for row in &current.rows {
            let mut cells = row.clone();
            for (i, field) in fields.iter().enumerate() {
                let value = row[i].as_str();
                let mapped = match (name.as_str(), field.as_str()) {
                    ("PROJECT" | "PROJWBS" | "SCHEDOPTIONS" | "TASK", "proj_id") => {
                        projects.get(value)
                    }
                    ("PROJECT", "clndr_id") if replays => project_calendar
                        .get(current.get(row, "proj_id"))
                        .cloned()
                        .unwrap_or_default(),
                    ("PROJWBS", "wbs_id" | "parent_wbs_id") | ("TASK", "wbs_id")
                        if !value.is_empty() =>
                    {
                        nodes.get(value)
                    }
                    ("TASK", "task_id") | ("TASKPRED", "task_id" | "pred_task_id") => {
                        tasks.get(value)
                    }
                    ("TASK", "clndr_id") if replays => task_calendar
                        .get(current.get(row, "task_id"))
                        .cloned()
                        .unwrap_or_default(),
                    ("TASKPRED", "task_pred_id") => {
                        let key = (
                            tasks.get(current.get(row, "task_id")),
                            tasks.get(current.get(row, "pred_task_id")),
                        );
                        link_ids.remove(&key).unwrap_or_else(|| {
                            next_link += 1;
                            next_link.to_string()
                        })
                    }
                    _ => continue,
                };
                cells[i] = mapped;
            }
            let cells: Vec<Cell<'_>> = cells.iter().map(|cell| Cell::Text(cell)).collect();
            writer.row(&cells).map_err(error)?;
        }
    }
    Ok(writer.finish())
}
