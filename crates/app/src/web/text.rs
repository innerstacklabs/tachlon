//! Read-only task text and JSON methods.

use super::App;
use crate::meta::{self, TaskMeta};
use engine::TaskId;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::JSON;

#[wasm_bindgen]
impl App {
    /// The task table's rows `first..last` (clamped to the schedule) as a
    /// JSON array of `[task, start, finish, float in minutes or null,
    /// critical, kind]`, where kind is 0 for a leaf, 1 for an expanded
    /// summary, and 2 for a collapsed summary.
    pub fn table_rows(&self, first: u32, last: u32) -> String {
        self.session.table_rows_json(first, last)
    }

    /// The full outline path of task `i` (`Level 3 / Concrete / Pour slab`), or an empty string if
    /// absent.
    pub fn task_name(&self, i: u32) -> String {
        self.session.outline_path(i)
    }

    /// The activity ID of task `i` as the ID column shows it (`A1010`), or an empty string when
    /// the task has none or is absent.
    pub fn task_code(&self, i: u32) -> String {
        self.session.task_code(i).to_string()
    }

    /// Whether any task of the open schedule has an activity ID. The table shows the ID column
    /// only then.
    pub fn has_codes(&self) -> bool {
        self.session.has_codes()
    }

    /// The WBS code of task `i` as the WBS column shows it (`HAND.1.2`), or an empty string when
    /// the task has none, is absent or is deleted.
    pub fn task_wbs_code(&self, i: u32) -> String {
        if !self.session.is_live(TaskId(i)) {
            return String::new();
        }
        self.session
            .meta()
            .get(i as usize)
            .map_or("", TaskMeta::wbs_code_text)
            .to_owned()
    }

    /// Whether any task of the open schedule has a WBS code. The table shows the WBS column only
    /// then. It scans every task, so the page asks once per opened file, not per frame.
    pub fn has_wbs_codes(&self) -> bool {
        meta::has_wbs_codes(self.session.meta())
    }

    /// The longest activity ID and the longest WBS code of the live tasks, as `[id, wbs]` (the
    /// first of equal length in task order; an empty string where no task has one). The table sizes
    /// its ID and WBS columns from them. It scans every task, so the page asks once per opened
    /// file, not per frame.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn longest_codes(&self) -> Vec<String> {
        let longest = |text: &dyn Fn(u32) -> String| {
            (0..self.session.meta().len() as u32)
                .map(text)
                .fold(String::new(), |best, next| {
                    if next.chars().count() > best.chars().count() {
                        next
                    } else {
                        best
                    }
                })
        };
        vec![
            longest(&|i| self.task_code(i)),
            longest(&|i| self.task_wbs_code(i)),
        ]
    }

    /// The longest own name (by characters, the first of equal length in task order) of the live
    /// tasks at each outline level, as an array where entry `n` is level `n + 1`; an empty string
    /// stands where a level has none. A name's width depends on its indent, so the table measures
    /// one name per level. It scans every task, so the page asks once per opened file, not per
    /// frame.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn longest_name(&self) -> Vec<String> {
        let mut longest: Vec<String> = Vec::new();
        for i in 0..self.session.meta().len() as u32 {
            let level = self.session.task_outline_level(i) as usize;
            if level == 0 {
                continue;
            }
            if longest.len() < level {
                longest.resize(level, String::new());
            }
            let name = self.session.task_label(i);
            if name.chars().count() > longest[level - 1].chars().count() {
                longest[level - 1] = name.to_string();
            }
        }
        longest
    }

    /// The file's data date as the table writes dates (`2026-10-14 08:00`), or an empty string
    /// when the schedule has none.
    pub fn data_date_text(&self) -> String {
        self.session.data_date_text().unwrap_or_default()
    }

    /// The own name of task `i` (its last outline segment), or an empty string if absent.
    pub fn task_label(&self, i: u32) -> String {
        self.session.task_label(i).to_string()
    }

    /// The duration of task `i` as the table shows it, or an empty string if
    /// absent.
    pub fn task_duration(&self, i: u32) -> String {
        self.session.task_duration(i)
    }

    /// The total float of task `i` as the table shows it, in working days of
    /// the task's calendar (`3d`, `2.5d`), or an empty string if absent.
    pub fn task_float(&self, i: u32) -> String {
        self.session.task_float(i)
    }

    /// The actual start of task `i` as the Start column writes dates (`2026-01-06 09:30`), or an
    /// empty string if it has none or is absent.
    pub fn task_actual_start(&self, i: u32) -> String {
        self.session.task_actual_start(i)
    }

    /// The actual finish of task `i` as the Finish column writes dates, or an empty string if it
    /// has none or is absent.
    pub fn task_actual_finish(&self, i: u32) -> String {
        self.session.task_actual_finish(i)
    }

    /// The remaining duration of task `i` in the Dur column's format, or an empty string for work
    /// not started or a task that is absent.
    pub fn task_remaining(&self, i: u32) -> String {
        self.session.task_remaining(i)
    }

    /// The day of limit `key` (`snlt`, `fnet` or `fnlt`) on task `i`, or an empty string.
    pub fn task_limit(&self, i: u32, key: &str) -> String {
        self.session.task_limit_text(i, key)
    }

    /// The percent complete of task `i` as a whole number with `%` (`75%`), or an empty string for
    /// work not started or a task that is absent.
    pub fn task_percent(&self, i: u32) -> String {
        self.session.task_percent(i)
    }

    /// Whether the open schedule carries progress. The table shows the four progress columns only
    /// then. It scans every task, so the page asks once per opened file, not per frame.
    pub fn has_progress(&self) -> bool {
        self.session.has_progress()
    }

    /// Everything the hover card shows for task `i`, as a plain object, or
    /// `undefined` past the schedule.
    pub fn task_details(&self, i: u32) -> JsValue {
        let Some(json) = self.session.task_details_json(i) else {
            return JsValue::UNDEFINED;
        };
        JSON::parse(&json).expect("task_details_json writes valid JSON")
    }

    /// The solved start of every live task as a JSON array of `YYYY-MM-DD HH:MM` texts, in the
    /// order `save` lists the tasks.
    pub fn task_starts(&self) -> String {
        self.session.task_starts_json()
    }

    /// The outline level (1 to 3) of task `i`, or zero if absent.
    pub fn task_outline_level(&self, i: u32) -> u8 {
        self.session.task_outline_level(i)
    }

    /// The number of tasks the baseline holds, or 0 when none is set.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn baseline_rows(&self) -> u32 {
        self.session.baseline().map_or(0, |b| b.len() as u32)
    }
}
