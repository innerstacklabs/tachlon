//! One WebGPU instance, adapter and device per page, shared by every chart that starts on WebGPU.
//! A chart on WebGL2 keeps its own instance: a WebGL2 context is bound to its canvas.
//!
//! The wasm module loads once per page, so this thread-local cache is page-wide. A chart that starts
//! while another is still waiting for its adapter waits for it and uses its device.

use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Poll, Waker};
use timeline::render::Gpu;
use web_sys::HtmlCanvasElement;

/// An instance and the device requested from it, kept alive by every chart that draws with them.
pub(super) struct Shared {
    instance: wgpu::Instance,
    pub(super) gpu: Gpu,
}

/// A chart's start: the shared handle and the chart's own surface on it.
pub(super) struct Started {
    pub(super) shared: Rc<Shared>,
    pub(super) surface: wgpu::Surface<'static>,
}

enum State {
    Idle,
    /// A chart is requesting the device; later ones wait.
    Pending(Vec<Waker>),
    Ready(Rc<Shared>),
}

thread_local! {
    static CACHE: RefCell<State> = const { RefCell::new(State::Idle) };
}

/// Holds the `Pending` state. Dropping it, when the start ends or is cancelled, caches `ready` (none
/// if the device is not shareable) and wakes the charts that waited.
#[derive(Default)]
struct Leader {
    ready: Option<Rc<Shared>>,
}

impl Drop for Leader {
    fn drop(&mut self) {
        let next = self.ready.take().map_or(State::Idle, State::Ready);
        let old = CACHE.with(|cache| std::mem::replace(&mut *cache.borrow_mut(), next));
        if let State::Pending(waiters) = old {
            waiters.into_iter().for_each(Waker::wake);
        }
    }
}

enum Claim {
    Hit(Rc<Shared>),
    Wait,
    Lead,
}

fn claim() -> Claim {
    CACHE.with(|cache| {
        let mut state = cache.borrow_mut();
        match &*state {
            // A device that has lost or errored is not reused: this chart asks for a fresh one.
            State::Ready(shared) if shared.gpu.error().is_none() => Claim::Hit(shared.clone()),
            State::Pending(_) => Claim::Wait,
            State::Idle | State::Ready(_) => {
                *state = State::Pending(Vec::new());
                Claim::Lead
            }
        }
    })
}

async fn settled() {
    std::future::poll_fn(|cx| {
        CACHE.with(|cache| match &mut *cache.borrow_mut() {
            State::Pending(waiters) => {
                waiters.push(cx.waker().clone());
                Poll::Pending
            }
            _ => Poll::Ready(()),
        })
    })
    .await;
}

fn surface(
    instance: &wgpu::Instance,
    canvas: &HtmlCanvasElement,
    size: (u32, u32),
) -> Result<wgpu::Surface<'static>, String> {
    canvas.set_width(size.0);
    canvas.set_height(size.1);
    instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("surface: {e}"))
}

/// A new instance, a surface on `canvas` and a device compatible with it.
async fn request(
    canvas: &HtmlCanvasElement,
    size: (u32, u32),
    force_webgl: bool,
) -> Result<Started, String> {
    let backends = if force_webgl {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
    .await;
    let surface = surface(&instance, canvas, size)?;
    // A lost device on charts that stopped drawing: tell the page, whose loops draw once more and see it.
    let on_lost = || {
        if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::Event::new("gpulost")) {
            let _ = window.dispatch_event(&event);
        }
    };
    let gpu = Gpu::request(&instance, &surface, on_lost).await?;
    let shared = Rc::new(Shared { instance, gpu });
    Ok(Started { shared, surface })
}

/// The instance, device and a surface on `canvas` (`size` physical pixels) for a chart. WebGPU is
/// shared with the page's other charts; `force_webgl`, or an adapter that is not WebGPU, gives the
/// chart a device of its own.
pub(super) async fn start(
    canvas: &HtmlCanvasElement,
    size: (u32, u32),
    force_webgl: bool,
) -> Result<Started, String> {
    if force_webgl {
        return request(canvas, size, true).await;
    }
    loop {
        match claim() {
            Claim::Hit(shared) => {
                let surface = surface(&shared.instance, canvas, size)?;
                return Ok(Started { shared, surface });
            }
            Claim::Wait => settled().await,
            Claim::Lead => {
                let mut leader = Leader::default();
                let started = request(canvas, size, false).await?;
                if started.shared.gpu.backend() == wgpu::Backend::BrowserWebGpu {
                    leader.ready = Some(started.shared.clone());
                }
                return Ok(started);
            }
        }
    }
}
