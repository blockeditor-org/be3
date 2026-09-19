use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use crate::color::Color32;
use crate::context::FrameOutput;
use crate::draw::{Quad, quads};
use crate::filter::Filter;
use crate::font::{GlyphId, GlyphImage};
use crate::geometry::{Rect, Vec2};

mod filter;

const ATLAS_SIZE: u32 = 2048;
const GLYPH_PADDING: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    clip: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
}

impl Instance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    screen: [f32; 2],
    padding: [f32; 2],
}

struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    entries: HashMap<GlyphId, [f32; 4]>,
    row_y: u32,
    row_height: u32,
    cursor_x: u32,
    full: bool,
}

impl Atlas {
    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            entries: HashMap::new(),
            row_y: 0,
            row_height: 0,
            cursor_x: 0,
            full: false,
        }
    }

    fn reset(&mut self) {
        self.entries.clear();
        self.row_y = 0;
        self.row_height = 0;
        self.cursor_x = 0;
        self.full = false;
    }

    fn insert(&mut self, queue: &wgpu::Queue, id: GlyphId, image: &GlyphImage) -> Option<[f32; 4]> {
        if let Some(uv) = self.entries.get(&id) {
            return Some(*uv);
        }
        let width = image.width + GLYPH_PADDING;
        let height = image.height + GLYPH_PADDING;
        if width > ATLAS_SIZE || height > ATLAS_SIZE {
            return None;
        }
        if self.cursor_x + width > ATLAS_SIZE {
            self.row_y += self.row_height;
            self.row_height = 0;
            self.cursor_x = 0;
        }
        if self.row_y + height > ATLAS_SIZE {
            self.full = true;
            return None;
        }

        let x = self.cursor_x;
        let y = self.row_y;
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );

        let scale = ATLAS_SIZE as f32;
        let uv = [
            x as f32 / scale,
            y as f32 / scale,
            (x + image.width) as f32 / scale,
            (y + image.height) as f32 / scale,
        ];
        self.entries.insert(id, uv);
        Some(uv)
    }
}

#[derive(Clone, Copy)]
pub enum Repaint {
    Everything,
    Region { region: Rect, background: Color32 },
}

impl Repaint {
    pub fn union(self, other: Self) -> Self {
        match (self, other) {
            (
                Self::Region { region, .. },
                Self::Region {
                    region: added,
                    background,
                },
            ) => Self::Region {
                region: region.union(added),
                background,
            },
            _ => Self::Everything,
        }
    }
}

pub struct Renderer {
    srgb: bool,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    punch_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    runs: Vec<Run>,
    overlay: Vec<Run>,
    scissor: Option<[u32; 4]>,
    atlas: Atlas,
    filter: Option<filter::Prepared>,
    effects: Option<filter::Effects>,
}

struct Run {
    punch: bool,
    start: u32,
    count: u32,
}

impl Run {
    fn push(runs: &mut Vec<Self>, punch: bool, at: u32) {
        match runs.last_mut() {
            Some(run) if run.punch == punch => run.count += 1,
            _ => runs.push(Self {
                punch,
                start: at,
                count: 1,
            }),
        }
    }
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let erase = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let punch_pipeline = pipeline(
            device,
            &layout,
            &shader,
            format,
            "beui punch pipeline",
            wgpu::BlendState {
                color: erase,
                alpha: erase,
            },
        );
        let pipeline = pipeline(
            device,
            &layout,
            &shader,
            format,
            "beui pipeline",
            wgpu::BlendState::ALPHA_BLENDING,
        );

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui instances"),
            size: (instance_capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = Atlas::new(device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui glyph sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &atlas,
            &sampler,
        );

        Self {
            srgb: format.is_srgb(),
            format,
            pipeline,
            punch_pipeline,
            bind_group,
            uniform_buffer,
            instance_buffer,
            instance_capacity,
            runs: Vec::new(),
            overlay: Vec::new(),
            scissor: None,
            atlas,
            filter: None,
            effects: None,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &FrameOutput,
        screen: Vec2,
        pixels_per_point: f32,
        repaint: Repaint,
    ) -> Repaint {
        if self.atlas.full {
            self.atlas.reset();
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                screen: [screen.x, screen.y],
                padding: [0.0, 0.0],
            }),
        );

        let prepared = output
            .filter()
            .map(|filter: Filter| filter::Prepared::new(&filter, pixels_per_point, screen));
        let repaint = match prepared == self.filter {
            true => repaint,
            false => Repaint::Everything,
        };
        let mut instances = Vec::new();
        let mut runs = Vec::new();
        let mut overlay = Vec::new();
        let damaged = match repaint {
            Repaint::Everything => None,
            Repaint::Region { region, background } => {
                let region = physical(region, screen, pixels_per_point);
                let region = match &prepared {
                    Some(prepared) => prepared.widen(region),
                    None => region,
                };
                Run::push(&mut runs, false, instances.len() as u32);
                instances.push(Instance {
                    rect: region,
                    clip: region,
                    uv: [0.0; 4],
                    color: self.encode(background),
                    params: [0.0, 0.0, 0.0, 0.0],
                });
                Some(region)
            }
        };
        let drawn = quads(output, pixels_per_point);
        let split = match prepared {
            Some(_) => drawn.filtered,
            None => drawn.list.len(),
        };
        for (index, quad) in drawn.list.into_iter().enumerate() {
            let layer = match index < split {
                true => &mut runs,
                false => &mut overlay,
            };
            match quad {
                Quad::Rect {
                    rect,
                    clip,
                    color,
                    corner_radius,
                    stroke_width,
                } => {
                    if skipped(damaged, expand(rect, stroke_width), clip) {
                        continue;
                    }
                    Run::push(layer, false, instances.len() as u32);
                    instances.push(Instance {
                        rect,
                        clip,
                        uv: [0.0; 4],
                        color: self.encode(color),
                        params: [corner_radius, stroke_width, 0.0, 0.0],
                    });
                }
                Quad::Glyph {
                    rect,
                    clip,
                    color,
                    glyph,
                } => {
                    if skipped(damaged, rect, clip) {
                        continue;
                    }
                    let Some(uv) = self.atlas.insert(queue, glyph.id, &glyph.image) else {
                        continue;
                    };
                    Run::push(layer, false, instances.len() as u32);
                    instances.push(Instance {
                        rect,
                        clip,
                        uv,
                        color: self.encode(color),
                        params: [0.0, 0.0, 1.0, 0.0],
                    });
                }
                Quad::Punch {
                    rect,
                    clip,
                    corner_radius,
                } => {
                    if skipped(damaged, rect, clip) {
                        continue;
                    }
                    Run::push(layer, true, instances.len() as u32);
                    instances.push(Instance {
                        rect,
                        clip,
                        uv: [0.0; 4],
                        color: [0.0, 0.0, 0.0, 1.0],
                        params: [corner_radius, 0.0, 0.0, 0.0],
                    });
                }
            }
        }

        self.scissor = damaged.map(scissor);
        self.runs = runs;
        self.overlay = overlay;
        self.filter = prepared;
        if instances.is_empty() {
            return repaint;
        }
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("beui instances"),
                size: (self.instance_capacity * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        repaint
    }

    fn encode(&self, color: Color32) -> [f32; 4] {
        match self.srgb {
            true => color.to_linear_f32(),
            false => color.to_normalized_gamma_f32(),
        }
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.clip(pass) {
            return;
        }
        self.draw(pass, &self.runs);
        self.draw(pass, &self.overlay);
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let Some(prepared) = self.filter else {
            if let Some(effects) = self.effects.as_mut() {
                effects.release();
            }
            let mut pass = self.begin(encoder, target, load);
            self.paint(&mut pass);
            return;
        };
        if self.empty() {
            return;
        }
        let (format, srgb, scissor) = (self.format, self.srgb, self.scissor);
        self.effects
            .get_or_insert_with(|| filter::Effects::new(device, format))
            .ensure(device, size);
        {
            let Some(scene) = self.effects.as_ref().and_then(filter::Effects::scene) else {
                return;
            };
            let mut pass = self.begin(encoder, scene, load);
            self.clip(&mut pass);
            self.draw(&mut pass, &self.runs);
        }
        if let Some(effects) = self.effects.as_mut() {
            effects.record(device, queue, encoder, &prepared, srgb, scissor);
        }
        let mut pass = self.begin(encoder, target, load);
        self.clip(&mut pass);
        if let Some(effects) = self.effects.as_ref() {
            effects.compose(&mut pass);
        }
        self.draw(&mut pass, &self.overlay);
    }

    fn empty(&self) -> bool {
        self.scissor
            .is_some_and(|[_, _, width, height]| width == 0 || height == 0)
    }

    fn clip(&self, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some([left, top, width, height]) = self.scissor else {
            return true;
        };
        if width == 0 || height == 0 {
            return false;
        }
        pass.set_scissor_rect(left, top, width, height);
        true
    }

    fn begin<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> wgpu::RenderPass<'pass> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, runs: &[Run]) {
        if runs.is_empty() {
            return;
        }
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        for run in runs {
            pass.set_pipeline(match run.punch {
                true => &self.punch_pipeline,
                false => &self.pipeline,
            });
            pass.draw(0..6, run.start..run.start + run.count);
        }
    }
}

fn physical(region: Rect, screen: Vec2, pixels_per_point: f32) -> [f32; 4] {
    [
        (region.left() * pixels_per_point)
            .floor()
            .clamp(0.0, screen.x),
        (region.top() * pixels_per_point)
            .floor()
            .clamp(0.0, screen.y),
        (region.right() * pixels_per_point)
            .ceil()
            .clamp(0.0, screen.x),
        (region.bottom() * pixels_per_point)
            .ceil()
            .clamp(0.0, screen.y),
    ]
}

fn expand(rect: [f32; 4], amount: f32) -> [f32; 4] {
    [
        rect[0] - amount,
        rect[1] - amount,
        rect[2] + amount,
        rect[3] + amount,
    ]
}

fn skipped(damaged: Option<[f32; 4]>, rect: [f32; 4], clip: [f32; 4]) -> bool {
    let Some(damaged) = damaged else {
        return false;
    };
    let left = rect[0].max(clip[0]).max(damaged[0]);
    let top = rect[1].max(clip[1]).max(damaged[1]);
    let right = rect[2].min(clip[2]).min(damaged[2]);
    let bottom = rect[3].min(clip[3]).min(damaged[3]);
    left >= right || top >= bottom
}

fn scissor(damaged: [f32; 4]) -> [u32; 4] {
    let width = (damaged[2] - damaged[0]).max(0.0) as u32;
    let height = (damaged[3] - damaged[1]).max(0.0) as u32;
    [damaged[0] as u32, damaged[1] as u32, width, height]
}

pub fn clear_color(color: Color32) -> wgpu::Color {
    let [red, green, blue, alpha] = color.to_linear_f32();
    wgpu::Color {
        r: red as f64,
        g: green as f64,
        b: blue as f64,
        a: alpha as f64,
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    label: &str,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Instance::layout()],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    atlas: &Atlas,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beui bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests;
