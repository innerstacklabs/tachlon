//! The wasm-bindgen API the web shell calls. Coordinates are CSS pixels
//! relative to the canvas's top-left corner.

#[cfg(feature = "alloc-stats")]
mod alloc_stats;
mod baseline;
mod card;
mod checks;
mod clipboard;
mod commands;
mod compare;
mod create;
mod export;
mod filters;
mod found;
mod gpu;
mod headless;
mod host;
mod input;
mod insert_many;
mod move_row;
mod offscreen;
mod print;
mod progress;
mod schedule;
mod shape;
mod start;
mod task_fields;
mod test_exports;
mod text;

use crate::{RenderFacts, Session};
use schedule::LoadedSchedule;
use timeline::render::Renderer;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use web_sys::js_sys::{Array, Object, Reflect};

/// The schedule and its timeline, drawn to one canvas.
#[wasm_bindgen]
pub struct App {
    canvas: HtmlCanvasElement,
    // The instance and device, kept for the lifetime of the surface (shared with the page's other WebGPU charts).
    _shared: std::rc::Rc<gpu::Shared>,
    renderer: Renderer,
    /// The schedule, its solution, the timeline and the drag path.
    session: Session,
    bars: usize,
    critical_bars: usize,
    arrows: usize,
    /// Non-working-day columns the last frame drew.
    shaded: usize,
    /// x of the data-date line in CSS pixels, if the last frame drew it.
    data_date_x: Option<f64>,
    /// The bar labels the last frame placed (see [`App::test_bar_labels`]).
    bar_labels: Vec<timeline::labels::BarLabel>,
    /// The bar rectangles the last frame drew, by task (see [`App::test_bar_rects`]).
    bar_rects: Vec<(u32, timeline::scene::Rect)>,
    frames: u64,
    /// The stress fixture's head task (a measured run drags it), if the
    /// schedule is the stress fixture.
    head: Option<u32>,
    /// Counts the files opened in place ([`App::open`]). The shell resets its own state when
    /// `Status.epoch` moves.
    epoch: u32,
    /// The file the schedule was read from, for [`App::save`]; none for a sample.
    source: Option<(crate::document::SourceFormat, Vec<u8>)>,
    /// The "unsupported features" lines the schedule was opened with, for [`App::save`].
    notes: Vec<String>,
    /// What this app's next frame does on purpose (`App.test_fail_next_frame`); one per app, so
    /// a test fails one chart of a page and not the others.
    fault: crate::frame_fault::FrameFault,
}

pub(super) fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// The first panic's message. A panic aborts the call that raised it and
/// leaves `App` borrowed, so the shell reads this to say what went wrong.
static PANIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Whether the next [`App::create`] deliberately panics during initialization.
pub(super) static PANIC_AT_INIT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            if let Ok(mut p) = PANIC.lock()
                && p.is_none()
            {
                *p = Some(info.to_string());
            }
            console_error_panic_hook::hook(info);
        }));
    });
}

/// The first panic's message, if the module has panicked.
#[wasm_bindgen]
pub fn last_panic() -> Option<String> {
    PANIC.lock().ok().and_then(|p| p.clone())
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "max(1.0) makes the value at least 1 (NaN becomes 1), so it is positive; a float-to-u32 `as` saturates, so a size past u32::MAX clamps and never wraps"
)]
fn physical(css: f64, dpr: f64) -> u32 {
    (css * dpr).round().max(1.0) as u32
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "every caller hands over the value it just built, and Reflect::set borrows it"
)]
pub(super) fn set(object: &Object, key: &str, value: JsValue) {
    Reflect::set(object.as_ref(), &JsValue::from_str(key), &value)
        .expect("a new plain object accepts properties");
}

pub(super) fn strings(values: &[String]) -> JsValue {
    let array: Array = values.iter().map(|v| JsValue::from_str(v)).collect();
    array.into()
}

#[wasm_bindgen]
impl App {
    /// Replace the schedule on the page with `schedule`, in place. The renderer, the canvas and
    /// its size stay. Everything that can fail is built first, so a failed open leaves the
    /// previous schedule as it was. A successful open starts a new session: no selection, no
    /// filter, no undo history, a view fitted to the new schedule. `Status.epoch` goes up by
    /// one, and the shell resets its own state when it sees that.
    pub fn open(&mut self, schedule: LoadedSchedule) -> Result<(), JsError> {
        install_panic_hook();
        let LoadedSchedule {
            schedule,
            solution,
            order,
            meta,
            outline,
            unsupported: notes,
            head,
            format: _,
            data_date,
            origin,
            timings: _,
            span: _,
            progress,
            deadlines,
            resources,
            source,
            baseline,
            collapsed,
            keys,
        } = schedule;
        let session = self
            .session
            .reopen(schedule, solution, order, meta, outline, data_date, origin)
            .map_err(|error| JsError::new(&error.to_string()))?
            .with_progress(progress);
        let session = schedule::restore(session, baseline, &collapsed, deadlines, resources, keys);
        self.session = session;
        // The bars of the first screen draw in (gs#309); reduced motion and a scroll snap them.
        self.session.mark_open();
        self.source = source;
        self.notes = notes;
        self.head = head;
        self.bars = 0;
        self.critical_bars = 0;
        self.arrows = 0;
        self.shaded = 0;
        self.data_date_x = None;
        self.bar_labels.clear();
        self.bar_rects.clear();
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
    }

    /// The session as a schedule document (the JSON text; the page gzips it). `name` is the
    /// schedule's name. It reads the session and changes nothing.
    pub fn save(&self, name: &str) -> String {
        let source = self
            .source
            .as_ref()
            .map(|(format, bytes)| crate::document::Source {
                format: *format,
                bytes,
            });
        crate::document::write(
            &self.session,
            &crate::document::Extras {
                name,
                notes: &self.notes,
                source,
            },
        )
    }

    /// Backend the page got: "WebGPU" or "WebGL2".
    pub fn backend(&self) -> String {
        self.renderer.backend_name().to_string()
    }

    /// Apply a partial JSON theme override to the current colours. Invalid
    /// JSON returns an error and leaves every colour unchanged.
    pub fn set_theme(&mut self, json: &str) -> Result<(), JsError> {
        let theme = self
            .session
            .theme()
            .with_json(json)
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.session.set_theme(theme);
        Ok(())
    }

    /// The complete JSON theme whose colours are used to build the scene.
    pub fn theme(&self) -> String {
        self.session.theme().to_json()
    }

    /// The stable JSON array of theme token keys accepted by [`App::set_theme`].
    pub fn theme_tokens() -> String {
        format!(r#"["{}"]"#, timeline::theme::TOKENS.join(r#"",""#))
    }

    /// Set the bar look from the `--gantt-bar-r` and `--gantt-bar-h` tokens: corner `radius` in
    /// CSS pixels and bar `height` as a share of the row. A bad value throws and changes nothing.
    pub fn set_bar_look(&mut self, radius: f64, height: f64) -> Result<(), JsError> {
        self.session
            .set_bar_look(radius, height)
            .map_err(|error| JsError::new(&error))
    }

    /// Set the row height by density name ("compact", "normal" or "roomy"); another name throws and changes nothing.
    pub fn set_density(&mut self, name: &str) -> Result<(), JsError> {
        self.session
            .set_density(name)
            .map_err(|error| JsError::new(&error))
    }

    /// Number of tasks.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "this module is built for wasm32 only, where usize is 32 bits, so a usize fits u32 exactly"
    )]
    pub fn task_count(&self) -> u32 {
        self.session.task_count() as u32
    }

    /// The canvas is now `css_width` x `css_height` at `dpr`.
    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        let (w, h) = (physical(css_width, dpr), physical(css_height, dpr));
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        self.renderer.resize(w, h);
        self.session.resize(css_width, css_height, dpr);
    }

    /// A key was pressed: stop a Fit or find that is flying, where it is.
    pub fn cancel_fly(&mut self) {
        self.session.cancel_fly();
    }

    /// Draw a frame. False if the surface skipped it.
    pub fn frame(&mut self) -> Result<bool, JsError> {
        self.fault.fire().map_err(JsError::new)?;
        self.session.apply_drag();
        // After the drag: a drag step makes a new solution, and a slide of the old stamp snaps here.
        self.session.advance_motion();
        // Before the scene: a flying frame draws the viewport the fly has reached.
        self.session.advance_fly();
        self.session.refresh_selection();
        self.session.advance_reveal();
        let renderer = &mut self.renderer;
        let mut scene = self
            .session
            .frame_scene(|task, text, px| renderer.measure_bar_label(task, text, px));
        self.bars = scene.bars;
        self.critical_bars = scene.critical_bars;
        self.arrows = scene.arrows;
        self.shaded = scene.shaded;
        self.data_date_x = scene.data_date_x;
        let drew = self
            .renderer
            .render(&scene, self.session.theme())
            .map_err(|e| JsError::new(&e))?;
        self.bar_labels = std::mem::take(&mut scene.bar_labels);
        self.bar_rects = scene
            .bar_rects
            .iter()
            .map(|&(task, index)| (task, scene.rects[index]))
            .collect();
        self.frames += u64::from(drew);
        Ok(drew)
    }

    /// State for the overlay and for tests, as JSON.
    pub fn status(&self) -> String {
        self.session.status_json(&RenderFacts {
            backend: self.renderer.backend_name(),
            surface: self.renderer.size(),
            bars: self.bars,
            critical_bars: self.critical_bars,
            arrows: self.arrows,
            shaded: self.shaded,
            data_date_x: self.data_date_x,
            epoch: self.epoch,
            frames: self.frames,
        })
    }

    /// Whether the Moved filter is on.
    pub fn moved_only(&self) -> bool {
        self.session.moved_only()
    }

    /// Turn the Moved filter on or off: a row shows only when it is a task the last move report
    /// lists, or a summary with one inside it. False when it cannot change now (no move report,
    /// no outline, or a bar press is active). Each new report moves it to that report's tasks; a
    /// change that makes none ends it. A successful change bumps [`Session::stamp`].
    pub fn set_moved_only(&mut self, on: bool) -> bool {
        self.session.set_moved_only(on)
    }
}
