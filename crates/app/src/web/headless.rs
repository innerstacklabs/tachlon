//! The six read calls with no canvas (gantt#1250): `Headless.open` makes a session from a read file and
//! no renderer, so a Node program asks `what_moves`, `what_moves_longer`, `explain`, `health`, `critical_path`
//! and `results` the way the page does (`packages/gantt/src/headless.ts` wraps them).

use super::schedule::{LoadedSchedule, restore};
use super::{install_panic_hook, now_ms};
use crate::Session;
use wasm_bindgen::prelude::*;

/// The size of the view the session is fitted to; no call here reads it, but a session has one.
const VIEW: (f64, f64) = (1200.0, 800.0);

/// A schedule's session with no canvas, renderer or page.
#[wasm_bindgen]
pub struct Headless {
    session: Session,
}

#[wasm_bindgen]
impl Headless {
    /// Open `schedule` (consumed, as `App.create` does) with no canvas. Throws `JsError` if the outline cannot be built.
    pub fn open(schedule: LoadedSchedule) -> Result<Headless, JsError> {
        install_panic_hook();
        let LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline,
            data_date,
            origin,
            progress,
            deadlines,
            resources,
            baseline,
            collapsed,
            keys,
            ..
        } = schedule;
        let mut timeline = crate::fitted_timeline(order, &solution, VIEW.0, VIEW.1, 1.0);
        timeline.set_progress(progress);
        let session = Session::new(schedule, solution, timeline, meta, now_ms)
            .with_data_date(data_date)
            .with_origin(origin);
        let session = match outline {
            Some(outline) => session
                .with_outline(outline)
                .map_err(|error| JsError::new(&error.to_string()))?,
            None => session,
        };
        let session = restore(session, baseline, &collapsed, deadlines, resources, keys);
        Ok(Headless { session })
    }

    /// `App.what_moves`: what moves if each of `tasks` (at most 10) slips a week, as the same JSON. Reads only.
    pub fn what_moves(&self, tasks: &[u32]) -> String {
        self.session.what_moves(tasks)
    }

    /// `App.what_moves_longer`: what moves if `task` takes `days` working days longer, as the same JSON. Reads only.
    pub fn what_moves_longer(&self, task: u32, days: u32) -> String {
        self.session.what_moves_longer(task, days)
    }

    /// `App.explain`: the tasks whose dependencies set `task`'s early start, as the same JSON. Reads only.
    pub fn explain(&self, task: u32) -> String {
        self.session.explain(task)
    }

    /// `App.health`: the tasks behind each `checks` count that walks tasks, as the same JSON. Reads only.
    pub fn health(&self) -> String {
        self.session.health()
    }

    /// `App.critical_path`: the critical tasks and the longest path's tasks, as the same JSON. Reads only.
    pub fn critical_path(&self) -> String {
        self.session.critical_path()
    }

    /// Every task's early and late dates, total and free float, and critical flag, as a JSON array (decision 0017's names, floats in working days). Throws `JsError` if the free floats cannot be worked out. Reads only.
    pub fn results(&self) -> Result<String, JsError> {
        self.session
            .results_json()
            .map_err(|error| JsError::new(&error.to_string()))
    }
}
