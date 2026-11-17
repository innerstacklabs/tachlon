//! The hover card's Calendar, Float in calendar days, Start no earlier than, Deadline, Driven by and activity code rows.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// The calendar of task `i` as `5 days a week, 8 h a day`, or an empty string if the task is
    /// absent or deleted.
    pub fn task_calendar_text(&self, i: u32) -> String {
        self.session.task_calendar_text(i)
    }
}

#[wasm_bindgen]
impl App {
    /// The total float of task `i` in calendar days as `7 calendar days`, or an empty string if
    /// it is under a day, or the task is absent or deleted.
    pub fn task_float_calendar_text(&self, i: u32) -> String {
        self.session.task_float_calendar_text(i)
    }
}

#[wasm_bindgen]
impl App {
    /// The start-no-earlier-than of task `i` as `YYYY-MM-DD HH:MM`, or an empty string if it has
    /// none, is absent or is deleted.
    pub fn task_constraint_text(&self, i: u32) -> String {
        self.session.task_constraint_text(i)
    }
}

#[wasm_bindgen]
impl App {
    /// The predecessors whose links set task `i`'s early start, one outline path a line, or an
    /// empty string if none does, the task is absent or is deleted.
    pub fn task_driven_by(&self, i: u32) -> String {
        self.session.task_driven_by(i)
    }
}

#[wasm_bindgen]
impl App {
    /// The MS Project deadline of task `i` as `YYYY-MM-DD HH:MM`, with `(missed by 2d)` when the
    /// task finishes after it, or an empty string if it has none, is absent or is deleted.
    pub fn task_deadline_text(&self, i: u32) -> String {
        self.session.task_deadline_text(i)
    }
}

#[wasm_bindgen]
impl App {
    /// The activity codes of task `i` as a JSON array of `[type, value]`, one card row each, or
    /// `[]` if it has none, is absent or is deleted.
    pub fn task_activity_codes(&self, i: u32) -> String {
        self.session.task_activity_codes(i)
    }
}
