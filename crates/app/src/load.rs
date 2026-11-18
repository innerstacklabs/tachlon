//! The load path: open a file into a solved schedule, with the timings of each phase.

mod baseline;

use crate::baseline::Baseline;
use crate::outline::{self, Outline};
use crate::{dates, load_order, meta};
use baseline::{file_baseline, file_dates, file_dates_line};
use engine::{Schedule, Solution, TaskId};

/// The demo schedule: `n` total tasks, solved, its row order (by early start),
/// and app-owned names and outline metadata for every task. Leaves come from
/// the generator and summaries are appended zero-duration milestones (see
/// [`outline::demo_layout`]).
#[must_use]
pub fn demo(n: usize) -> (Schedule, Solution, Vec<u32>, Vec<meta::TaskMeta>) {
    let layout = outline::demo_layout(n);
    (layout.schedule, layout.solution, layout.order, layout.meta)
}

/// A schedule loaded from an MS Project XML or Primavera P6 XER file, ready for `Session::new`.
pub struct Loaded {
    pub schedule: Schedule,
    pub solution: Solution,
    /// Row order (by early start), as `demo` returns it.
    pub order: Vec<u32>,
    pub meta: Vec<meta::TaskMeta>,
    /// The validated task forest, in file order; install it with [`Session::with_outline`](crate::Session::with_outline).
    pub outline: Outline,
    /// `Imported::unsupported`, then the file-dates line, then one line per dependency dropped
    /// because it touches a summary task.
    pub unsupported: Vec<String>,
    /// The file's data date as an instant (`Imported::data_date`): the MS Project `StatusDate` or the P6 project's `last_recalc_date`; `None` when the file states none.
    pub data_date: Option<engine::Instant>,
    /// The calendar date of instant 0, from the file (`Imported::origin_day`). Install it with
    /// [`Session::with_origin`](crate::Session::with_origin); every date text goes through it.
    pub origin: dates::Origin,
    /// Percent complete (0 to 100) per task in file order, 0 for a task with none; display only.
    /// Install it with [`Session::with_progress`](crate::Session::with_progress).
    pub progress: Vec<u8>,
    /// The MS Project deadline per task in file order, `None` for a task with none; display only.
    /// Install it with [`Session::with_deadlines`](crate::Session::with_deadlines).
    pub deadlines: Vec<Option<engine::Instant>>,
    /// The resource names per task in file order, empty for a task with none; display only.
    /// Install it with [`Session::with_assignments`](crate::Session::with_assignments).
    pub resources: Vec<Vec<crate::Assigned>>,
    /// The file's own baseline (MS Project's Baseline 0), when a task states one; install it with
    /// [`Session::with_baseline`](crate::Session::with_baseline). Its entries hold no float.
    pub baseline: Option<Baseline>,
    /// How long each phase of the load took, from the caller's clock.
    pub timings: OpenTimings,
}

/// Milliseconds spent in each phase of [`load_bytes`], read from the clock the caller passes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OpenTimings {
    /// Detect the format, read the file and map it into the engine schedule (the importer).
    pub parse: f64,
    /// Build the app's display data from the import, set the actuals and free the import's task list.
    pub build: f64,
    /// `Schedule::compute`, the row order, the outline and the check of in-progress file dates against the solution.
    pub compute: f64,
}

impl OpenTimings {
    /// The phases and the independently measured `span` as a JSON object,
    /// `{"parse":..,"build":..,"compute":..,"span":..}`, in milliseconds. A value that is not a finite
    /// number is written as `null`, so the text is always valid JSON and the page can tell an invalid
    /// measurement from a real one. A negative value is written as it is: nothing is clamped to zero.
    pub fn to_json(&self, span: f64) -> String {
        let [parse, build, compute, span] =
            [self.parse, self.build, self.compute, span].map(json_ms);
        format!(r#"{{"parse":{parse},"build":{build},"compute":{compute},"span":{span}}}"#)
    }
}

/// A millisecond value as a JSON number, or `null` when it is NaN or infinite.
fn json_ms(ms: f64) -> String {
    if ms.is_finite() {
        ms.to_string()
    } else {
        "null".to_string()
    }
}

/// Why [`load_bytes`] did not return a schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The XER file holds several projects and the selection named none. The projects are in file
    /// order; the caller picks one (or all) and loads again.
    ChooseProject(Vec<import::XerProject>),
    /// The file could not be loaded; the text is the importer's or the engine's message.
    Message(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::ChooseProject(projects) => {
                write!(f, "the file holds {} projects, choose one", projects.len())
            }
            OpenError::Message(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for OpenError {}

/// Read the project selection the page passes with a file (gs#87): `None` asks for the only
/// project (and fails with [`OpenError::ChooseProject`] when there are several), `"all"` opens
/// every project, and any other text is one P6 `proj_id`. The text is the page's own, so a bad
/// one is a good-faith mistake and gets a message.
///
/// # Errors
/// A message naming the text when it is neither `all` nor a project id.
pub fn parse_selection(text: Option<&str>) -> Result<import::ProjectSelection, String> {
    match text {
        None => Ok(import::ProjectSelection::Unspecified),
        Some("all") => Ok(import::ProjectSelection::All),
        Some(id) => id
            .trim()
            .parse::<u32>()
            .map(|id| import::ProjectSelection::Only(vec![id]))
            .map_err(|_| format!("the project selection `{id}` is neither `all` nor a project id")),
    }
}

/// Parse an MS Project XML or Primavera P6 XER document into a solved schedule.
///
/// A file that starts with `ERMHDR` (after an optional byte order mark) goes to
/// `import::parse_xer` with `selection`, any other file to `import::parse_mspdi`.
/// An XER file with several projects and `ProjectSelection::Unspecified` returns
/// [`OpenError::ChooseProject`] listing the projects in file order.
///
/// `now` reads a clock in milliseconds (the browser passes `performance.now`; wasm32 cannot call
/// `std::time::Instant`) and fills [`Loaded::timings`]. It is called four times, in order:
/// before the import, after it, after the display data and outline, after the solve.
///
/// The function holds no state, so a failed load leaves whatever schedule the caller has
/// already loaded as it was.
///
/// Per gs#47 ruling 5, every dependency that touches a summary task (a task
/// that some other task names as its parent) is dropped and listed in
/// [`Loaded::unsupported`]. The error text is the importer's or the engine's
/// message, so a malformed file is a message, never a panic.
///
/// # Errors
/// [`OpenError::ChooseProject`] for an XER file with several projects and no selection, else an
/// [`OpenError::Message`] with the importer's or the engine's message.
#[expect(clippy::too_many_lines, reason = "load steps run in one fixed order")]
pub fn load_bytes(
    bytes: &[u8],
    selection: &import::ProjectSelection,
    now: &dyn Fn() -> f64,
) -> Result<Loaded, OpenError> {
    let started = now();
    let import::Imported {
        mut schedule,
        tasks,
        mut unsupported,
        data_date,
        origin_day,
        ..
    } = import::parse_bytes(bytes, selection).map_err(|error| match error {
        import::ImportError::ProjectChoice { projects } => OpenError::ChooseProject(projects),
        other => OpenError::Message(other.to_string()),
    })?;
    let parsed = now();
    let task_count = tasks.len();
    if task_count == 0 {
        return Err(OpenError::Message("the file has no tasks".to_string()));
    }

    let mut summaries = vec![false; task_count];
    for task in &tasks {
        if let Some(parent) = task.parent
            && let Some(summary) = summaries.get_mut(parent.0 as usize)
        {
            *summary = true;
        }
    }

    // The importer's schedule is the one the app keeps, with each link that touches a summary taken
    // out in place: the link keeps its slot, which every reader of the dependencies skips.
    let mut dropped = Vec::new();
    let mut touching = Vec::new();
    for (id, dependency) in schedule.live_dependencies() {
        let pred_summary = summaries
            .get(dependency.pred.0 as usize)
            .copied()
            .unwrap_or(false);
        let succ_summary = summaries
            .get(dependency.succ.0 as usize)
            .copied()
            .unwrap_or(false);
        if pred_summary || succ_summary {
            let pred = tasks
                .get(dependency.pred.0 as usize)
                .ok_or_else(|| failed(format!("unknown task {}", dependency.pred.0)))?;
            let succ = tasks
                .get(dependency.succ.0 as usize)
                .ok_or_else(|| failed(format!("unknown task {}", dependency.succ.0)))?;
            let summary_uid = if pred_summary { pred.uid } else { succ.uid };
            dropped.push(format!(
                "dependency {} -> {} ({:?}) dropped: task {} is a summary task",
                pred.uid, succ.uid, dependency.kind, summary_uid
            ));
            touching.push(id);
        }
    }
    for id in touching {
        schedule.remove_dependency(id).map_err(failed)?;
    }

    // The data date and the actuals (decision 0008) shape the first compute, as they do in P6.
    schedule.set_data_date(data_date).map_err(failed)?;
    for (index, task) in tasks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
        )]
        let id = TaskId(index as u32);
        schedule
            .set_actual_start(id, task.actual_start)
            .and_then(|()| schedule.set_actual_finish(id, task.actual_finish))
            .and_then(|()| schedule.set_remaining(id, task.remaining))
            .map_err(failed)?;
    }

    // What the load keeps of the tasks beyond their names is read here, then the names move into the
    // display data and the import's task list is freed before the solve: freeing 50,000 tasks takes
    // time, and after the solve it would fall outside the phases.
    let progress: Vec<u8> = tasks
        .iter()
        .map(|task| task.progress.unwrap_or(0))
        .collect();
    let deadlines = tasks.iter().map(|task| task.deadline).collect();
    let resources = tasks
        .iter()
        .map(|task| crate::resources::full_units(task.resource_names.clone()))
        .collect();
    let baseline = file_baseline(&tasks);
    let file_dates = file_dates(&tasks);
    let meta: Vec<meta::TaskMeta> = tasks
        .into_iter()
        .map(|task| meta::TaskMeta {
            name: leaf_name(task.uid, task.name),
            parent: task.parent,
            outline_level: task.outline_level,
            code: task.code,
            activity_codes: task.activity_codes,
            wbs_code: task.wbs_code,
            uid: Some(task.uid),
            floor: task.floor,
            color: None,
            label: None,
            key: None,
        })
        .collect();
    let built = now();
    let solution = schedule.compute().map_err(failed)?;
    let order = timeline::order_by_start(&solution.dates);
    // The rows under a WBS node follow the Start column, which is the solved start (gs#865).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    let wbs: Vec<TaskId> = (0..task_count).map(|index| TaskId(index as u32)).collect();
    let wbs = load_order::by_computed_start(&wbs, &meta, &solution.dates);
    let outline = outline::Outline::new(&meta, &wbs, &schedule).map_err(failed)?;

    unsupported.extend(file_dates_line(&file_dates, &solution));
    unsupported.extend(dropped);
    let origin = dates::Origin::from_day(origin_day);
    let solved = now();

    Ok(Loaded {
        schedule,
        solution,
        order,
        meta,
        outline,
        unsupported,
        data_date,
        origin,
        progress,
        deadlines,
        resources,
        baseline,
        timings: OpenTimings {
            parse: elapsed(started, parsed),
            build: elapsed(parsed, built),
            compute: elapsed(built, solved),
        },
    })
}

/// The name an imported task keeps: the file's text byte for byte, except that a control character
/// becomes a space and a blank name becomes `Task <uid>`. A `" / "` in the name stays.
fn leaf_name(uid: u32, name: String) -> String {
    if name.trim().is_empty() {
        return format!("Task {uid}");
    }
    if !name.chars().any(char::is_control) {
        return name;
    }
    name.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn failed(error: impl std::fmt::Display) -> OpenError {
    OpenError::Message(error.to_string())
}

/// Milliseconds from `from` to `to`. A clock that goes backwards gives a negative value and one that
/// returns NaN gives NaN: an invalid measurement stays invalid, and the page rejects it, rather than
/// being turned into a believable 0.
fn elapsed(from: f64, to: f64) -> f64 {
    to - from
}

/// [`load_bytes`] with a second, independent measure of its length: the clock is read once before the
/// call and once after it, and the span between them is returned beside the result. The three phases of
/// [`OpenTimings`] come from reads inside `load_bytes`; the span comes from two reads outside it, so a
/// phase that loses or double-counts time disagrees with the span instead of being absorbed by it.
/// The page uses [`load_bytes_with_span`] instead (gs#248).
///
/// # Errors
/// The [`OpenError`] of [`load_bytes`].
pub fn timed_load_bytes(
    bytes: &[u8],
    selection: &import::ProjectSelection,
    now: &dyn Fn() -> f64,
) -> Result<(Loaded, f64), OpenError> {
    let entered = now();
    let loaded = load_bytes(bytes, selection, now)?;
    let left = now();
    Ok((loaded, elapsed(entered, left)))
}

/// [`load_bytes`] with the length of its whole run, from the same clock reads as the phases: the first
/// read to the last. The span equals the sum of the phases up to float rounding, however long the process
/// was paused between two reads. (`timed_load_bytes` took two more reads outside; a pause between those
/// and the inner reads counted in the span and in no phase, and under CPU load failed the page's sum
/// check, gs#248.) A read added outside the three windows moves the first or last read and shows as a gap.
///
/// # Errors
/// The [`OpenError`] of [`load_bytes`].
pub fn load_bytes_with_span(
    bytes: &[u8],
    selection: &import::ProjectSelection,
    now: &dyn Fn() -> f64,
) -> Result<(Loaded, f64), OpenError> {
    let first = std::cell::Cell::new(None);
    let last = std::cell::Cell::new(f64::NAN);
    let recording = || {
        let at = now();
        first.set(first.get().or(Some(at)));
        last.set(at);
        at
    };
    let loaded = load_bytes(bytes, selection, &recording)?;
    Ok((loaded, elapsed(first.get().unwrap_or(f64::NAN), last.get())))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use xer::synth::{Spec, write};

    fn clock_of<'a>(readings: &'static [f64], next: &'a Cell<usize>) -> impl Fn() -> f64 + 'a {
        move || {
            let at = next.get();
            next.set(at + 1);
            readings[at]
        }
    }

    #[test]
    fn the_span_comes_from_the_same_reads_as_the_phases() {
        // However long the process stalled between two reads, the phases and the span agree: the span is
        // the first inner read to the last, so a 500 ms stall lands in a phase and in the span alike.
        static READINGS: [f64; 4] = [3.0, 13.0, 15.0, 540.0];
        let next = Cell::new(0);
        let clock = clock_of(&READINGS, &next);
        let (loaded, span) = load_bytes_with_span(
            &write(&Spec::small(5)),
            &import::ProjectSelection::Unspecified,
            &clock,
        )
        .expect("loads");
        assert_eq!(next.get(), 4, "no read outside the four of load_bytes");
        let timings = loaded.timings;
        assert_eq!(
            (timings.parse, timings.build, timings.compute),
            (10.0, 2.0, 525.0)
        );
        assert_eq!(span, 537.0);
        assert_eq!(span, timings.parse + timings.build + timings.compute);
    }

    #[test]
    fn a_clock_that_is_not_a_number_gives_a_span_that_is_not_a_number() {
        let (_, span) = load_bytes_with_span(
            &write(&Spec::small(5)),
            &import::ProjectSelection::Unspecified,
            &|| f64::NAN,
        )
        .expect("loads");
        assert!(span.is_nan());
    }

    #[test]
    fn a_failed_load_returns_the_error_and_no_span() {
        let error = load_bytes_with_span(
            b"not a schedule",
            &import::ProjectSelection::Unspecified,
            &|| 0.0,
        )
        .map(|_| ())
        .expect_err("fails");
        assert!(matches!(error, OpenError::Message(_)));
    }
}
