//! wgpu renderer: one instanced-quad pipeline for every rectangle in a
//! [`Scene`] (bars, grid, axis band), then glyphon for the axis labels.
//!
//! The caller creates the [`wgpu::Instance`] and the surface (the app does
//! it from a canvas), so this module has no web dependencies and compiles
//! natively too.

mod pipeline;

use crate::Theme;
use crate::scene::Scene;
use crate::text::{self, AxisText, BarText, Text, font_system};
use glyphon::{Cache, ColorMode, SwashCache, TextAtlas, TextRenderer, Viewport as TextViewport};
use pipeline::{Rects, instance_buffer};
use std::sync::{Arc, Mutex};

/// Keep the first error written to `slot`; later ones are usually fallout.
fn record(slot: &Mutex<Option<String>>, what: String) {
    if let Ok(mut s) = slot.lock()
        && s.is_none()
    {
        *s = Some(what);
    }
}

/// Draws [`Scene`]s to a surface.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    backend: wgpu::Backend,
    max_dim: u32,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: usize,
    text: Text,
    /// The first uncaptured device error or device loss, if any.
    gpu_error: Arc<Mutex<Option<String>>>,
}

/// The graphics device: adapter, device and queue, plus the slot that holds
/// the first device error. Cheap to clone (wgpu handles).
#[derive(Clone)]
pub struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    backend: wgpu::Backend,
    max_dim: u32,
    first_error: Arc<Mutex<Option<String>>>,
}

impl Gpu {
    /// The backend the adapter runs on.
    #[must_use]
    pub fn backend(&self) -> wgpu::Backend {
        self.backend
    }

    /// The first uncaptured error or device loss on this device, if any.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.first_error.lock().ok().and_then(|s| s.clone())
    }

    /// Request an adapter compatible with `surface` (created from `instance`),
    /// then a device and queue, and watch the device for errors and loss.
    /// `on_lost` runs when the device is lost, so a caller that stopped drawing can draw once more and see it.
    ///
    /// # Errors
    ///
    /// Returns a message when the instance has no adapter for `surface`, or the adapter gives no device.
    pub async fn request(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'static>,
        on_lost: impl Fn() + Send + 'static,
    ) -> Result<Gpu, String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(surface),
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| format!("no adapter: {e}"))?;
        let backend = adapter.get_info().backend;
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let max_dim = limits.max_texture_dimension_2d;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("timeline"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no device: {e}"))?;
        let gpu_error = Arc::new(Mutex::new(None));
        let slot = gpu_error.clone();
        device.on_uncaptured_error(Arc::new(move |e| record(&slot, format!("GPU error: {e}"))));
        let slot = gpu_error.clone();
        device.set_device_lost_callback(move |reason, msg| {
            record(&slot, format!("GPU device lost ({reason:?}): {msg}"));
            on_lost();
        });
        Ok(Gpu {
            adapter,
            device,
            queue,
            backend,
            max_dim,
            first_error: gpu_error,
        })
    }
}

impl Renderer {
    /// A renderer for `surface` (created from the instance `gpu` was
    /// requested with), `width` x `height` physical pixels.
    ///
    /// # Errors
    ///
    /// Returns a message when the surface reports no formats.
    pub fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Renderer, String> {
        let Gpu {
            adapter,
            device,
            queue,
            backend,
            max_dim,
            first_error: gpu_error,
        } = gpu.clone();
        let caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB target: colours are sRGB values already.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("surface reports no formats")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.clamp(1, max_dim),
            height: height.clamp(1, max_dim),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let Rects {
            pipeline,
            uniforms,
            bind_group,
        } = Rects::new(&device, format);
        let capacity = 1024;
        let instances = instance_buffer(&device, capacity);

        let fonts = font_system();
        let cache = Cache::new(&device);
        let color_mode = if format.is_srgb() {
            ColorMode::Accurate
        } else {
            ColorMode::Web
        };
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, color_mode);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let text = Text {
            fonts,
            swash: SwashCache::new(),
            viewport: TextViewport::new(&device, &cache),
            atlas,
            renderer: text_renderer,
            axis: AxisText::default(),
            bars: BarText::default(),
        };

        Ok(Renderer {
            surface,
            device,
            queue,
            config,
            backend,
            max_dim,
            pipeline,
            uniforms,
            bind_group,
            instances,
            capacity,
            text,
            gpu_error,
        })
    }

    /// The first uncaptured GPU error or device loss since creation, if
    /// any. The web shell uses it to fall back from WebGPU to WebGL2.
    pub fn gpu_error(&self) -> Option<String> {
        self.gpu_error.lock().ok().and_then(|s| s.clone())
    }

    /// Backend name: "WebGPU", "WebGL2", "Vulkan", ...
    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            wgpu::Backend::Gl if cfg!(target_arch = "wasm32") => "WebGL2",
            wgpu::Backend::Gl => "GL",
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "DX12",
            wgpu::Backend::Noop => "Noop",
        }
    }

    /// The width in physical pixels of the bar label `text` of `task` at font
    /// size `px` physical pixels. The text is shaped on the first call and
    /// kept while the task keeps being measured every frame, so a view that
    /// does not change its labels shapes nothing.
    pub fn measure_bar_label(&mut self, task: u32, text: &str, px: f32) -> f32 {
        self.text.bars.measure(&mut self.text.fonts, task, text, px)
    }

    /// Add font faces and set the locale tag; cached labels are shaped again on the next frame.
    pub fn add_fonts(&mut self, locale: &str, faces: Vec<Vec<u8>>) -> usize {
        text::add_fonts(
            &mut self.text.fonts,
            &mut self.text.axis,
            &mut self.text.bars,
            locale,
            faces,
        )
    }

    /// Surface size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Resize the surface to `width` x `height` physical pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.clamp(1, self.max_dim), height.clamp(1, self.max_dim));
        if (w, h) != (self.config.width, self.config.height) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Draw `scene`. Returns false if the frame was skipped (surface busy or
    /// being reconfigured).
    ///
    /// # Errors
    ///
    /// Returns the device error once the device has reported one.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "w and h are the surface size, at most the GPU's texture limit (tens of thousands of pixels), far under f32's 2^24 exact range; the rects fill the instance buffer, which wgpu caps at max_buffer_size (256 MiB by default), far under u32::MAX instances"
    )]
    pub fn render(&mut self, scene: &Scene, theme: &Theme) -> Result<bool, String> {
        if let Some(e) = self.gpu_error() {
            return Err(e);
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            other => return Err(format!("surface: {other:?}")),
        };
        let (w, h) = (self.config.width, self.config.height);
        let srgb = if self.config.format.is_srgb() {
            1.0
        } else {
            0.0
        };
        self.queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::cast_slice(&[w as f32, h as f32, srgb, 0.0]),
        );
        if scene.rects.len() > self.capacity {
            self.capacity = scene.rects.len().next_power_of_two();
            self.instances = instance_buffer(&self.device, self.capacity);
        }
        self.queue
            .write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.rects));
        self.text
            .prepare(&self.device, &self.queue, scene, (w, h), theme)?;

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("timeline"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(theme.bg[0]),
                            g: f64::from(theme.bg[1]),
                            b: f64::from(theme.bg[2]),
                            a: f64::from(theme.bg[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !scene.rects.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instances.slice(..));
                pass.draw(0..6, 0..scene.rects.len() as u32);
            }
            self.text
                .renderer
                .render(&self.text.atlas, &self.text.viewport, &mut pass)
                .map_err(|e| format!("text render: {e}"))?;
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.text.atlas.trim();
        self.text.bars.end_frame();
        Ok(true)
    }
}
