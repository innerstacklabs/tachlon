//! `App::create`: the renderer and the first session for a schedule, on a canvas.

use super::schedule::{self, LoadedSchedule};
use super::{App, PANIC_AT_INIT, gpu, install_panic_hook, now_ms, physical};
use crate::Session;
use timeline::render::Renderer;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen]
impl App {
    /// Set up the renderer for `schedule` on `canvas` (`css_width` x
    /// `css_height` at `dpr`). WebGPU if the page gets an adapter, else WebGL2;
    /// `force_webgl` skips WebGPU.
    pub async fn create(
        canvas: HtmlCanvasElement,
        css_width: f64,
        css_height: f64,
        dpr: f64,
        force_webgl: bool,
        schedule: LoadedSchedule,
    ) -> Result<App, JsError> {
        install_panic_hook();
        assert!(
            !PANIC_AT_INIT.swap(false, std::sync::atomic::Ordering::SeqCst),
            "deliberate panic during init (?panic=init)"
        );
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
        let mut timeline = crate::fitted_timeline(order, &solution, css_width, css_height, dpr);
        timeline.set_progress(progress);

        let (w, h) = (physical(css_width, dpr), physical(css_height, dpr));
        let gpu::Started { shared, surface } = gpu::start(&canvas, (w, h), force_webgl)
            .await
            .map_err(|e| JsError::new(&e))?;
        let renderer = Renderer::new(&shared.gpu, surface, w, h).map_err(|e| JsError::new(&e))?;
        let session = Session::new(schedule, solution, timeline, meta, now_ms)
            .with_data_date(data_date)
            .with_origin(origin);
        let session = if let Some(outline) = outline {
            session
                .with_outline(outline)
                .map_err(|error| JsError::new(&error.to_string()))?
        } else {
            session
        };
        let session = schedule::restore(session, baseline, &collapsed, deadlines, resources, keys);
        Ok(App {
            canvas,
            _shared: shared,
            renderer,
            session,
            bars: 0,
            critical_bars: 0,
            arrows: 0,
            shaded: 0,
            data_date_x: None,
            bar_labels: Vec::new(),
            bar_rects: Vec::new(),
            frames: 0,
            head,
            epoch: 0,
            source,
            notes,
            fault: crate::frame_fault::FrameFault::None,
        })
    }
}
