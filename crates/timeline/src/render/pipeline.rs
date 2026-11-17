//! The rectangle pipeline: shader, uniform buffer, bind group and the
//! instance buffer the rectangles are uploaded to.

use crate::scene::Rect;

const SHADER: &str = r"
struct Uniforms { size: vec2<f32>, srgb_out: f32, _pad: f32 };
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Offset from the quad's top-left corner, its size, its corner radius (-1 marks a
    // diamond, -2 and -3 a triangle) and its ring width (0 fills the quad), physical pixels.
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) size: vec2<f32>,
    @location(3) @interpolate(flat) radius: f32,
    @location(4) @interpolate(flat) stroke: f32,
};

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
    @location(4) stroke: f32,
) -> VOut {
    // Two triangles: corners (0,0) (1,0) (0,1) (0,1) (1,0) (1,1).
    let corner = vec2<f32>(f32((vi == 1u) || (vi == 4u) || (vi == 5u)),
                           f32((vi == 2u) || (vi == 3u) || (vi == 5u)));
    let p = pos + corner * size;
    var out: VOut;
    out.clip = vec4<f32>(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
    // Colours are sRGB; an sRGB target expects linear values from the shader.
    let rgb = select(color.rgb, to_linear(color.rgb), u.srgb_out > 0.5);
    out.color = vec4<f32>(rgb, color.a);
    out.local = corner * size;
    out.size = size;
    out.radius = radius;
    out.stroke = stroke;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    if (in.radius == 0.0 && in.stroke <= 0.0) {
        return in.color;
    }
    let half_size = in.size * 0.5;
    if (in.radius < 0.0) {
        if (in.radius < -1.5) {
            // An arrowhead: a triangle in the quad, its base on the left edge and its tip in the
            // middle of the right (-2), or mirrored (-3). Each edge distance is in pixels along
            // its normal, so the antialiasing is one pixel wide, as for the diamond.
            let px = select(in.size.x - in.local.x, in.local.x, in.radius > -2.5);
            let n = length(vec2<f32>(in.size.x, half_size.y));
            let dist = max(max(-px, (px * half_size.y - in.local.y * in.size.x) / n),
                           (px * half_size.y + (in.local.y - in.size.y) * in.size.x) / n);
            return vec4<f32>(in.color.rgb, in.color.a * clamp(0.5 - dist, 0.0, 1.0));
        }
        // A diamond inscribed in the quad: its points touch the middle of each side. The distance
        // is scaled to pixels along the edge normal, so the antialiasing is one pixel wide.
        let d = abs(in.local - half_size) / half_size;
        let n = length(vec2<f32>(half_size.y, half_size.x));
        let dist = ((d.x + d.y - 1.0) * half_size.x * half_size.y) / n;
        var cover = clamp(0.5 - dist, 0.0, 1.0);
        if (in.stroke > 0.0) {
            // A dashed ring, as below; each edge spans the half height once, so `d.y` runs along it.
            // An edge holds `k` whole periods, with a dash centred on each point, so no point
            // sits in a gap; an edge shorter than two periods (a diamond of the normal row size)
            // draws solid, where one dash would leave a bracket at each point.
            let k = floor(n / (5.0 * in.stroke) + 0.5);
            cover = cover * clamp(0.5 + dist + in.stroke, 0.0, 1.0)
                * select(1.0, step(fract(d.y * k + 0.3), 0.6), k >= 2.0);
        }
        return vec4<f32>(in.color.rgb, in.color.a * cover);
    }
    // Signed distance to a rounded box centred on the quad; the radius cannot pass half the shorter side.
    let r = min(in.radius, min(half_size.x, half_size.y));
    let q = abs(in.local - half_size) - half_size + vec2<f32>(r);
    let dist = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    if (in.stroke > 0.0) {
        // A dashed ring: the band just inside the edge, `stroke` wide. Dashing by x + y moves the
        // phase along x on a horizontal edge and along y on a vertical one. A dash is 60% of a
        // period of five strokes.
        let outer = clamp(0.5 - dist, 0.0, 1.0);
        let inner = clamp(0.5 + dist + in.stroke, 0.0, 1.0);
        let dash = step(fract((in.local.x + in.local.y) / (5.0 * in.stroke)), 0.6);
        return vec4<f32>(in.color.rgb, in.color.a * outer * inner * dash);
    }
    // One pixel of antialiasing across the edge.
    return vec4<f32>(in.color.rgb, in.color.a * clamp(0.5 - dist, 0.0, 1.0));
}
";

/// The pipeline and the uniform buffer and bind group it reads.
pub(super) struct Rects {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) uniforms: wgpu::Buffer,
    pub(super) bind_group: wgpu::BindGroup,
}

impl Rects {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Rects {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rects"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rects"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32, 4 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rects"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Rect>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Rects {
            pipeline,
            uniforms,
            bind_group,
        }
    }
}

pub(super) fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rects"),
        size: (capacity * size_of::<Rect>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
