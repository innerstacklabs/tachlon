//! [`read`]: check the `format` and the `version`, then rebuild a [`Loaded`] from the fields in
//! layout order. Nothing here panics on a bad text: every refusal is a [`DocumentError`] that
//! names the value or the path (`tasks[12].remaining`) it could not read.

use super::fields::{
    Field, decode, fields, fields_allowing, int, list, opt, small, text_of, within,
};
use super::json_read::{Value, parse_streaming};
use super::{Document, DocumentError, FORMAT, SourceFormat, VERSION};
mod baseline;
mod build;
mod finish_by;
mod floor;
mod items;
mod limit;
mod options;
use crate::load::OpenTimings;
use crate::outline::Outline;
use crate::{Loaded, dates};
use baseline::read_baseline;
use build::Build;
use engine::TaskId;
use finish_by::read_finish_by;
use options::read_options;

const TOP: [&str; 13] = [
    "format",
    "version",
    "name",
    "origin",
    "data_date",
    "lag_calendar",
    "calendars",
    "tasks",
    "links",
    "baseline",
    "view",
    "notes",
    "source",
];

/// The document `text` holds. `now` is a milliseconds clock (as for [`load_bytes`](crate::load_bytes)),
/// called four times; the JSON read, in which the calendars, tasks and links are built as they are read,
/// is the `parse` phase of [`Loaded::timings`].
///
/// The `format` is checked first, then the `version`, then the fields in layout order. The first
/// field that is missing, unknown or of the wrong type is refused with its path, and an engine or
/// outline refusal (an unknown calendar, a parent cycle, a link to itself) with its message. A
/// parent that names no task is refused once every task is read, and an end of a link that names
/// none when the link is added, so an unreadable field in a later task, or later in the same task
/// or link, is named first.
///
/// # Errors
/// A [`DocumentError`] naming the first value the reader could not accept.
#[expect(clippy::too_many_lines, reason = "fields are read in layout order")]
pub fn read(text: impl Into<String>, now: &dyn Fn() -> f64) -> Result<Document, DocumentError> {
    let text = text.into();
    let started = now();
    let mut reading = Build::new(text.len());
    let root = parse_streaming(&text, &mut |members, key, index, end, item| {
        reading.feed(members, key, index, end, &item);
    })
    .map_err(DocumentError::Syntax)?;
    let parsed = now();
    match root.get("format") {
        Some(Value::Str(format)) if format == FORMAT => {}
        Some(other) => {
            return Err(DocumentError::NotDocument(format!(
                "format is {}",
                other.describe()
            )));
        }
        None => {
            return Err(DocumentError::NotDocument(
                "it has no \"format\" key".to_string(),
            ));
        }
    }
    match root.get("version") {
        Some(Value::Int(version)) if *version > i64::from(VERSION) => {
            return Err(DocumentError::Newer(*version));
        }
        Some(Value::Int(version)) if *version == i64::from(VERSION) => {}
        Some(other) => {
            return Err(invalid(format!(
                "version: expected {VERSION}, found {}",
                other.describe()
            )));
        }
        None => return Err(invalid("version: missing key".to_string())),
    }
    let [
        _,
        _,
        name,
        origin,
        data_date,
        _,
        _,
        _,
        _,
        baseline,
        view,
        notes,
        source,
    ] = fields_allowing(&root, &TOP, &["options", "must_finish_by"]).map_err(invalid)?;
    let name = text_of(name, "name").map_err(invalid)?;
    let origin = int(origin, "origin").map_err(invalid)?;
    let data_date = opt(data_date, |v| int(v, "data_date")).map_err(invalid)?;
    let notes: Vec<String> = list(notes, "notes")
        .and_then(|items| items.iter().map(|n| text_of(n, "notes[]")).collect())
        .map_err(invalid)?;

    let Build {
        mut schedule,
        meta,
        progress,
        keys,
        ..
    } = reading.finish(&root)?;
    let count = meta.len();

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the task slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    let wbs: Vec<TaskId> = (0..count as u32).map(TaskId).collect();
    let outline =
        Outline::new(&meta, &wbs, &schedule).map_err(|e| invalid(format!("outline: {e}")))?;
    let baseline =
        opt(baseline, |v| read_baseline(v, count)).map_err(|e| invalid(within("baseline", &e)))?;
    let collapsed = read_collapsed(view, &outline).map_err(invalid)?;
    let source = opt(source, read_source).map_err(|e| invalid(within("source", &e)))?;
    let options = read_options(&root).map_err(invalid)?;
    let finish_by = read_finish_by(&root).map_err(invalid)?;

    schedule
        .set_data_date(data_date)
        .map_err(|e| invalid(format!("data_date: {e}")))?;
    schedule.set_options(options);
    schedule.set_finish_by(finish_by);
    // Everything read from the tree is owned now. Wasm memory never shrinks, so the tree's bytes
    // and the text it borrows from would otherwise stay in the peak that the solve sets.
    drop(root);
    drop(text);
    let built = now();
    let solution = schedule
        .compute()
        .map_err(|e| invalid(format!("schedule: {e}")))?;
    let order = timeline::order_by_start(&solution.dates);
    let solved = now();

    Ok(Document {
        loaded: Loaded {
            schedule,
            solution,
            order,
            meta,
            outline,
            unsupported: notes.clone(),
            data_date,
            origin: dates::Origin::from_day(origin),
            progress,
            deadlines: Vec::new(),
            resources: Vec::new(),
            baseline: None,
            timings: OpenTimings {
                parse: (parsed - started).max(0.0),
                build: (built - parsed).max(0.0),
                compute: (solved - built).max(0.0),
            },
        },
        name,
        notes,
        baseline,
        collapsed,
        source,
        keys,
    })
}

fn invalid(message: String) -> DocumentError {
    DocumentError::Invalid(message)
}

fn read_collapsed(view: &Value, outline: &Outline) -> Result<Vec<TaskId>, String> {
    let [collapsed] = fields(view, &["collapsed"]).map_err(|e| within("view", &e))?;
    let mut out = Vec::new();
    for (i, id) in list(collapsed, "collapsed")
        .map_err(|e| within("view", &e))?
        .iter()
        .enumerate()
    {
        let at = |e: String| format!("view.collapsed[{i}]: {e}");
        let task = TaskId(small(id, "id").map_err(at)?);
        // A flag outlives the children of its task (a reparent can empty a collapsed summary), and
        // the writer keeps writing it, so only a task that does not exist is refused.
        if task.0 as usize >= outline.len() {
            return Err(format!("view.collapsed[{i}]: no task {}", task.0));
        }
        out.push(task);
    }
    Ok(out)
}

fn read_source(value: &Value) -> Field<(SourceFormat, Vec<u8>)> {
    let [format, data] = fields(value, &["format", "base64"])?;
    let format = match format.as_str() {
        Some("xer") => SourceFormat::Xer,
        Some("mspdi") => SourceFormat::Mspdi,
        _ => {
            return Err(format!(
                "format: expected \"xer\" or \"mspdi\", found {}",
                format.describe()
            ));
        }
    };
    let coded = data
        .as_str()
        .ok_or_else(|| format!("base64: expected a string, found {}", data.describe()))?;
    let bytes = decode(coded).map_err(|at| format!("base64: not base64 at character {at}"))?;
    Ok((format, bytes))
}
