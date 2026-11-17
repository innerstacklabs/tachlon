//! Schedule loading and parsing for the wasm API.

use super::{install_panic_hook, now_ms, set, strings};
use crate::baseline::Baseline;
use crate::document::SourceFormat;
use engine::TaskId;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Array, Error, Object};

/// The JS value `LoadedSchedule::parse_bytes` throws: the project list for a file that needs a
/// choice, an `Error` with the importer's text for anything else.
fn open_error(error: crate::OpenError) -> JsValue {
    match error {
        crate::OpenError::ChooseProject(projects) => {
            let thrown = Error::new(&format!(
                "the file holds {} projects, choose one",
                projects.len()
            ));
            thrown.set_name("ChooseProject");
            let list = Array::new();
            for project in &projects {
                let row = Object::new();
                set(&row, "id", JsValue::from_str(&project.proj_id.to_string()));
                set(
                    &row,
                    "name",
                    JsValue::from_str(project.short_name.as_deref().unwrap_or("")),
                );
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "activities is the task count of one project in a file, far under 2^53, so f64 holds it exactly"
                )]
                set(
                    &row,
                    "activities",
                    JsValue::from_f64(project.activities as f64),
                );
                list.push(&row);
            }
            set(&thrown, "projects", list.into());
            thrown.into()
        }
        crate::OpenError::Message(text) => JsError::new(&text).into(),
    }
}

/// A solved schedule and its display data, ready to pass to [`App::create`].
#[wasm_bindgen]
pub struct LoadedSchedule {
    pub(super) schedule: engine::Schedule,
    pub(super) solution: engine::Solution,
    pub(super) order: Vec<u32>,
    pub(super) meta: Vec<crate::meta::TaskMeta>,
    pub(super) outline: Option<crate::outline::Outline>,
    pub(super) unsupported: Vec<String>,
    pub(super) head: Option<u32>,
    /// `"xer"`, `"mspdi"`, `"gantt"` (a schedule document), `"demo"` or `"stress"`.
    pub(super) format: &'static str,
    pub(super) data_date: Option<engine::Instant>,
    /// The calendar date of instant 0: the file's (`Loaded::origin`), or the demo's.
    pub(super) origin: crate::dates::Origin,
    /// What each phase of `parse_bytes` took on the page's clock; all zero for the demo and stress fixtures.
    pub(super) timings: crate::OpenTimings,
    /// The first to the last clock read of `load_bytes` (see `crate::load_bytes_with_span`): the same reads as the
    /// three phases, so it equals their sum; zero for the demo and stress fixtures.
    pub(super) span: f64,
    /// Percent complete per task, 0 to 100; empty when the schedule has none (the demo has a synthetic spread).
    pub(super) progress: Vec<u8>,
    /// The MS Project deadline per task, display only (`Loaded::deadlines`); a document holds none.
    pub(super) deadlines: Vec<Option<engine::Instant>>,
    /// The resource names per task, display only (`Loaded::resources`); a document holds none.
    pub(super) resources: Vec<Vec<crate::Assigned>>,
    /// The file the schedule was read from, kept so a save can write it into the document.
    pub(super) source: Option<(SourceFormat, Vec<u8>)>,
    /// The baseline a document holds, indexed by the schedule's ids.
    pub(super) baseline: Option<Baseline>,
    /// The summaries a document holds collapsed.
    pub(super) collapsed: Vec<TaskId>,
    /// The keys a document holds for its links and calendars, and the maps over every key.
    pub(super) keys: crate::host::Keys,
}

#[wasm_bindgen]
impl LoadedSchedule {
    /// Parse the bytes of an MS Project XML or Primavera P6 XER file into a solved schedule.
    /// A file that starts with `ERMHDR` is XER; any other file is MS Project XML. `selection`
    /// is `undefined`, `"all"` or a P6 `proj_id`. An XER file with several projects and no
    /// selection throws an `Error` named `ChooseProject` that carries `projects`
    /// (`[{ id, name, activities }]`, in file order); every other failure is a plain `Error`.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen reads the optional JS string into an owned value"
    )]
    pub fn parse_bytes(
        bytes: Vec<u8>,
        selection: Option<String>,
    ) -> Result<LoadedSchedule, JsValue> {
        install_panic_hook();
        let selection = crate::parse_selection(selection.as_deref())
            .map_err(|error| JsValue::from(JsError::new(&error)))?;
        let (format, kind) = if crate::is_xer(&bytes) {
            ("xer", SourceFormat::Xer)
        } else {
            ("mspdi", SourceFormat::Mspdi)
        };
        let (
            crate::Loaded {
                schedule,
                solution,
                order,
                meta,
                outline,
                unsupported,
                data_date,
                origin,
                timings,
                progress,
                deadlines,
                resources,
                baseline: file_baseline,
            },
            span,
        ) = crate::load_bytes_with_span(&bytes, &selection, &now_ms).map_err(open_error)?;
        Ok(LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline: Some(outline),
            unsupported,
            head: None,
            format,
            data_date,
            origin,
            timings,
            span,
            progress,
            deadlines,
            resources,
            source: Some((kind, bytes)),
            baseline: file_baseline,
            collapsed: Vec::new(),
            keys: crate::host::Keys::default(),
        })
    }

    /// Read the text of a schedule document into a solved schedule. Throws a `JsError` with the
    /// reader's message when the text is not a document this build reads.
    pub fn parse_document(text: String) -> Result<LoadedSchedule, JsError> {
        install_panic_hook();
        let document = crate::document::read(text, &now_ms)
            .map_err(|error| JsError::new(&error.to_string()))?;
        let crate::document::Document {
            loaded,
            baseline,
            collapsed,
            source,
            keys,
            ..
        } = document;
        let crate::Loaded {
            schedule,
            solution,
            order,
            meta,
            outline,
            unsupported,
            data_date,
            origin,
            timings,
            progress,
            deadlines,
            resources,
            baseline: _,
        } = loaded;
        Ok(LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline: Some(outline),
            unsupported,
            head: None,
            format: crate::document::FORMAT,
            data_date,
            origin,
            span: timings.parse + timings.build + timings.compute,
            timings,
            progress,
            deadlines,
            resources,
            source,
            baseline,
            collapsed,
            keys,
        })
    }

    /// A solved schedule with no tasks and one Monday-to-Friday calendar, whose origin is the
    /// Monday on or before `start_day`, a day count since 1970-01-01.
    pub fn blank(start_day: i32) -> LoadedSchedule {
        install_panic_hook();
        let crate::Loaded {
            schedule,
            solution,
            order,
            meta,
            outline,
            unsupported,
            data_date,
            origin,
            timings,
            progress,
            deadlines,
            resources,
            baseline,
        } = crate::Loaded::blank(i64::from(start_day));
        LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline: Some(outline),
            unsupported,
            head: None,
            format: "blank",
            data_date,
            origin,
            timings,
            span: 0.0,
            progress,
            deadlines,
            resources,
            source: None,
            baseline,
            collapsed: Vec::new(),
            keys: crate::host::Keys::default(),
        }
    }

    /// Build the solved demo schedule with `n` tasks.
    pub fn demo(n: u32) -> Result<LoadedSchedule, JsError> {
        install_panic_hook();
        let layout = crate::outline::demo_layout(n as usize);
        let outline = crate::outline::Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
            .map_err(|error| JsError::new(&error.to_string()))?;
        let progress = crate::meta::demo_progress(layout.meta.len());
        Ok(LoadedSchedule {
            schedule: layout.schedule,
            solution: layout.solution,
            order: layout.order,
            meta: layout.meta,
            outline: Some(outline),
            unsupported: Vec::new(),
            head: None,
            format: "demo",
            data_date: None,
            origin: crate::dates::Origin::DEMO,
            timings: crate::OpenTimings::default(),
            span: 0.0,
            progress,
            deadlines: Vec::new(),
            resources: Vec::new(),
            source: None,
            baseline: None,
            collapsed: Vec::new(),
            keys: crate::host::Keys::default(),
        })
    }

    /// Build the solved stress fixture with `n` tasks.
    pub fn stress(n: u32) -> LoadedSchedule {
        install_panic_hook();
        let (mut schedule, head) = engine::stress(crate::SEED, (n as usize).max(2));
        let solution = schedule
            .compute()
            .expect("generated schedules are acyclic and valid");
        let order = timeline::order_by_start(&solution.dates);
        let mut meta = crate::names::generate(crate::SEED, schedule.tasks().len());
        crate::names::paint(&mut meta, crate::SEED);
        LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline: None,
            unsupported: Vec::new(),
            head: Some(head.0),
            format: "stress",
            data_date: None,
            origin: crate::dates::Origin::DEMO,
            timings: crate::OpenTimings::default(),
            span: 0.0,
            progress: Vec::new(),
            deadlines: Vec::new(),
            resources: Vec::new(),
            source: None,
            baseline: None,
            collapsed: Vec::new(),
            keys: crate::host::Keys::default(),
        }
    }

    /// Build the solved stress fixture with `n` tasks under one root summary, each coded `Phase` with one of `groups`
    /// values, so `set_group_by("Phase")` accepts it.
    pub fn stress_grouped(n: u32, groups: u32) -> Result<LoadedSchedule, JsError> {
        install_panic_hook();
        let (layout, head) = crate::outline::stress_grouped_layout(n as usize, groups as usize);
        let outline = crate::outline::Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
            .map_err(|error| JsError::new(&error.to_string()))?;
        Ok(LoadedSchedule {
            schedule: layout.schedule,
            solution: layout.solution,
            order: layout.order,
            meta: layout.meta,
            outline: Some(outline),
            unsupported: Vec::new(),
            head: Some(head.0),
            format: "stress",
            data_date: None,
            origin: crate::dates::Origin::DEMO,
            timings: crate::OpenTimings::default(),
            span: 0.0,
            progress: Vec::new(),
            deadlines: Vec::new(),
            resources: Vec::new(),
            source: None,
            baseline: None,
            collapsed: Vec::new(),
            keys: crate::host::Keys::default(),
        })
    }

    /// Unsupported features reported while importing the schedule.
    pub fn unsupported(&self) -> JsValue {
        strings(&self.unsupported)
    }

    /// What the schedule came from: `"xer"`, `"mspdi"`, `"gantt"` (a schedule document), `"demo"` or `"stress"`.
    pub fn format(&self) -> String {
        self.format.to_string()
    }

    /// How long `parse_bytes` spent in each Rust phase, in milliseconds, as JSON:
    /// `{"parse":..,"build":..,"compute":..,"span":..}`. `parse` is format detection, the reader and the
    /// mapping into the engine schedule; `build` is the display data, the outline and the engine schedule;
    /// `compute` is the solve, the row order and the in-progress date check, and freeing the import. The
    /// page's clock times them, so the page sees the same milliseconds it measures around the call.
    /// `span` is the first of those clock reads to the last, so it equals the sum of the three up to float
    /// rounding, however long the process was paused between two reads; the page and `measure.mjs` check that.
    /// A value that is not a finite number is `null`, and a negative one is kept as it is.
    /// All zero for `demo` and `stress`.
    pub fn timings(&self) -> String {
        self.timings.to_json(self.span)
    }
}

/// Put a document's baseline and collapsed summaries, and a file's deadlines and resource names, in force on a new
/// session. None is an edit, so the history stays empty.
pub(super) fn restore(
    session: crate::Session,
    baseline: Option<Baseline>,
    collapsed: &[TaskId],
    deadlines: Vec<Option<engine::Instant>>,
    resources: Vec<Vec<crate::Assigned>>,
    keys: crate::host::Keys,
) -> crate::Session {
    let session = match baseline {
        Some(baseline) => session.with_baseline(baseline),
        None => session,
    };
    session
        .with_deadlines(deadlines)
        .with_assignments(resources)
        .with_host_keys(keys)
        .with_collapsed(collapsed)
}
