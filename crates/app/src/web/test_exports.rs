//! Test-only wasm exports.

use super::{App, PANIC_AT_INIT, set};
use engine::TaskId;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Array, Object};

/// Test hook: the next `App::create` panics on purpose so the shell's init
/// watchdog can be exercised; one shot.
#[wasm_bindgen]
pub fn test_panic_at_init() {
    PANIC_AT_INIT.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[wasm_bindgen]
impl App {
    /// Test hook: this app's next `App.frame` fails on purpose so the shell's frame guard can be
    /// exercised; one shot. Another app on the page draws as usual.
    pub fn test_fail_next_frame(&mut self, panic: bool) {
        self.fault.arm(panic);
    }

    /// The generator seed of the schedule.
    #[expect(
        clippy::cast_precision_loss,
        reason = "SEED is the constant 20_261_020, exact in f64"
    )]
    #[expect(
        clippy::unused_self,
        reason = "an exported method, as the page calls it on an app"
    )]
    pub fn test_seed(&self) -> f64 {
        crate::SEED as f64
    }

    /// Whether this module was built with debug assertions (a debug build),
    /// which a measured run records: only a release build gives results.
    #[expect(
        clippy::unused_self,
        reason = "an exported method, as the page calls it on an app"
    )]
    pub fn test_debug_build(&self) -> bool {
        cfg!(debug_assertions)
    }

    /// The row of the stress fixture's head task, if this is the stress
    /// fixture.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn test_head_row(&self) -> Option<u32> {
        let head = self.head?;
        self.session.timeline().row_of_task(head).map(|r| r as u32)
    }

    /// Number of rows on screen, group headers included.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn test_row_count(&self) -> u32 {
        self.session.timeline().rows().len() as u32
    }

    /// The duration of task `i` in working minutes, or zero if absent.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a duration is minutes; past 2^53 (about 17 billion years) f64 would round it, and the JavaScript test reads it as a number, which has the same limit"
    )]
    pub fn test_task_minutes(&self, i: u32) -> f64 {
        self.session
            .schedule()
            .tasks()
            .get(i as usize)
            .map_or(0.0, |task| task.duration as f64)
    }

    /// The latest early finish over all tasks, in minutes.
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the origin; past 2^53 f64 would round it, and the JavaScript test reads it as a number, which has the same limit"
    )]
    pub fn test_project_finish(&self) -> f64 {
        self.session
            .solution()
            .dates
            .iter()
            .map(|dates| dates.ef)
            .max()
            .unwrap_or(0) as f64
    }

    /// The lowest critical task id whose early finish is the project finish.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn test_finishing_task(&self) -> Option<u32> {
        let finish = self
            .session
            .solution()
            .dates
            .iter()
            .map(|dates| dates.ef)
            .max()?;
        self.session
            .solution()
            .dates
            .iter()
            .position(|dates| dates.ef == finish && dates.is_critical())
            .map(|task| task as u32)
    }

    /// Read-only test helper: the tasks on the longest path
    /// (`Schedule::longest_path`), ascending ids, summaries excluded.
    pub fn test_longest_path_tasks(&self) -> Vec<u32> {
        let outline = self.session.outline();
        self.session
            .schedule()
            .longest_path(self.session.solution())
            .into_iter()
            .filter(|&task| !outline.is_some_and(|outline| outline.is_summary(task)))
            .map(|task| task.0)
            .collect()
    }

    /// The table row occupied by task `i`, if it exists.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn test_task_row(&self, i: u32) -> Option<u32> {
        self.session.timeline().row_of_task(i).map(|row| row as u32)
    }

    /// Read-only test helper: add working `minutes` to `from` using task
    /// `i`'s calendar.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "from and minutes come from JavaScript numbers, and a float-to-i64 `as` saturates (NaN becomes 0), so it never wraps; the i64 result is minutes, which f64 rounds past 2^53, the same limit as a JavaScript number"
    )]
    pub fn test_calendar_add(&self, i: u32, from: f64, minutes: f64) -> f64 {
        self.session
            .schedule()
            .calendar_of(TaskId(i))
            .add(from as i64, minutes as i64) as f64
    }

    /// The first row whose task is critical (zero float), if any. The
    /// verify script scrolls there to check that critical bars render.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn test_first_critical_row(&self) -> Option<u32> {
        self.session
            .timeline()
            .rows()
            .iter()
            .position(|&t| {
                t != timeline::HIDDEN_ROW && self.session.solution().dates[t as usize].is_critical()
            })
            .map(|r| r as u32)
    }

    /// The bar labels the last frame placed, for tests: an array of
    /// `{task, text, placement, x, y, right, textW, gap}` (x, y, right in physical
    /// pixels; textW and gap in CSS pixels; gap is the room right of the bar for the name, less the link terminal's reach for a task bar, so a name goes right when `textW + 8 <= gap`; placement is "inside" or "right"). Read from the placement, not from pixels.
    pub fn test_bar_labels(&self) -> JsValue {
        let array = Array::new();
        for l in &self.bar_labels {
            let object = Object::new();
            set(&object, "task", JsValue::from_f64(f64::from(l.task)));
            set(&object, "text", JsValue::from_str(&l.text));
            set(&object, "placement", JsValue::from_str(l.placement.name()));
            set(&object, "x", JsValue::from_f64(f64::from(l.x)));
            set(&object, "y", JsValue::from_f64(f64::from(l.y)));
            set(&object, "right", JsValue::from_f64(f64::from(l.right)));
            set(&object, "textW", JsValue::from_f64(f64::from(l.text_w)));
            set(&object, "gap", JsValue::from_f64(f64::from(l.gap)));
            array.push(&object);
        }
        array.into()
    }

    /// The bar rectangles the last frame drew, for tests: an array of
    /// `{task, x, y, width, height}` in physical pixels, one per task bar and summary bar
    /// (the scene's `bar_rects`, not recomputed from dates). A summary's rectangle is its top rule.
    pub fn test_bar_rects(&self) -> JsValue {
        let array = Array::new();
        for (task, rect) in &self.bar_rects {
            let object = Object::new();
            set(&object, "task", JsValue::from_f64(f64::from(*task)));
            set(&object, "x", JsValue::from_f64(f64::from(rect.pos[0])));
            set(&object, "y", JsValue::from_f64(f64::from(rect.pos[1])));
            set(&object, "width", JsValue::from_f64(f64::from(rect.size[0])));
            set(
                &object,
                "height",
                JsValue::from_f64(f64::from(rect.size[1])),
            );
            array.push(&object);
        }
        array.into()
    }

    /// The selected task and its driving path as JSON.
    pub fn test_selection_json(&self) -> String {
        let Some(selected) = self.session.selection() else {
            return "null".to_string();
        };
        let back = selected
            .back
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let forward = selected
            .forward
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                r#"{{"task":{},"row":{},"back":[{}],"forward":[{}],"backCapped":{},"forwardCapped":{}}}"#
            ),
            selected.task,
            selected.row,
            back,
            forward,
            selected.back_capped,
            selected.forward_capped
        )
    }
}

/// The counting allocator's numbers, in a profile build only: with the `alloc-stats` feature (`build-web.sh --keep-names`)
/// this export exists, and without it the shipped wasm has neither the export nor the allocator.
#[cfg(feature = "alloc-stats")]
#[wasm_bindgen]
impl App {
    /// `[peak live bytes, allocations, live bytes, memory.buffer bytes]`.
    pub fn test_alloc_stats(&self) -> Vec<f64> {
        use super::alloc_stats::{LIVE, PEAK, TOTAL};
        use std::sync::atomic::Ordering::Relaxed;
        let pages = core::arch::wasm32::memory_size(0) as f64;
        vec![
            PEAK.load(Relaxed) as f64,
            TOTAL.load(Relaxed) as f64,
            LIVE.load(Relaxed) as f64,
            pages * 65536.0,
        ]
    }
}
