use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use crate::color::Color32;
use crate::context::FrameOutput;
use crate::damage::Region;
use crate::display::{Display, Layer, Part};
use crate::draw::{Quad, Turn, push_quads};
use crate::painter::{Entry, placed_shape};
use crate::drawing::{DrawAt, Drawing};
use crate::filter::Filter;
use crate::font::{GlyphId, GlyphImage};
use crate::geometry::{Rect, Vec2};
use crate::image::{Image, ImageId};

mod filter;

#[derive(Clone, Debug, PartialEq)]
pub struct RendererInfo {
    pub adapter: wgpu::AdapterInfo,
    pub format: wgpu::TextureFormat,
}

impl RendererInfo {
    pub(crate) fn rows(&self) -> Vec<(&'static str, String)> {
        let adapter = &self.adapter;
        let mut rows = vec![
            ("Backend", format!("{:?}", adapter.backend)),
            ("Adapter", adapter.name.clone()),
            ("Device type", format!("{:?}", adapter.device_type)),
            ("Driver", or_unknown(&adapter.driver)),
            ("Driver info", or_unknown(&adapter.driver_info)),
            (
                "Vendor / device",
                format!("{:#06x} / {:#06x}", adapter.vendor, adapter.device),
            ),
        ];
        if !adapter.device_pci_bus_id.is_empty() {
            rows.push(("PCI bus", adapter.device_pci_bus_id.clone()));
        }
        rows.push(("Surface format", format!("{:?}", self.format)));
        rows
    }
}

fn or_unknown(value: &str) -> String {
    if value.is_empty() {
        "unknown".to_owned()
    } else {
        value.to_owned()
    }
}

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
    turn: [f32; 4],
}

impl Instance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4
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
    origin: [f32; 2],
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
    Region { region: Region, background: Color32 },
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

impl FrameOutput {
    pub fn repaint(&self, background: Color32) -> Repaint {
        match self.damage.is_empty() {
            true => Repaint::Everything,
            false => Repaint::Region {
                region: self.damage,
                background,
            },
        }
    }
}

struct Picture {
    bind_groups: [wgpu::BindGroup; 2],
    used: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Space {
    translation: [f32; 2],
    padding: [f32; 2],
    clip: [f32; 4],
}

const OPEN: [f32; 4] = [-1.0e9, -1.0e9, 1.0e9, 1.0e9];
const SPACES: usize = 64;
const LISTED: usize = 4096;
const REACH: f32 = 2.0;

pub struct Renderer {
    srgb: bool,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    punch_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    space_layout: wgpu::BindGroupLayout,
    space_buffer: wgpu::Buffer,
    space_group: wgpu::BindGroup,
    space_capacity: usize,
    space_stride: u64,
    list_buffer: wgpu::Buffer,
    list_capacity: usize,
    list_top: usize,
    generation: u64,
    lists: HashMap<ListKey, Encoded>,
    live: usize,
    atlas_epoch: u64,
    #[cfg(test)]
    pub(crate) encoded: usize,
    runs: Vec<Run>,
    overlay: Vec<Run>,
    scissors: Option<Vec<[u32; 4]>>,
    origin: Vec2,
    atlas: Atlas,
    pictures: HashMap<ImageId, Picture>,
    samplers: [wgpu::Sampler; 2],
    filter: Option<filter::Prepared>,
    effects: Option<filter::Effects>,
    bounds: Option<[u32; 4]>,
    whole: bool,
}

struct Run {
    punch: bool,
    picture: Option<(ImageId, bool)>,
    drawing: Option<(Drawing, DrawAt)>,
    listed: bool,
    space: u32,
    start: u32,
    count: u32,
}

impl Run {
    fn push(runs: &mut Vec<Self>, run: Self) {
        if let Some(last) = runs.last_mut()
            && last.drawing.is_none()
            && run.drawing.is_none()
            && last.punch == run.punch
            && last.picture == run.picture
            && last.listed == run.listed
            && last.space == run.space
            && last.start + last.count == run.start
        {
            last.count += run.count;
            return;
        }
        runs.push(run);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ListKey {
    display: u64,
    factor: u32,
    offset: [u32; 2],
    clip: [u32; 4],
}

struct Encoded {
    instances: Vec<Instance>,
    runs: Vec<ListRun>,
    parts: Vec<(usize, usize)>,
    placed: Option<(u64, u32)>,
    atlas: u64,
}

enum ListRun {
    Quads {
        punch: bool,
        picture: Option<(Image, bool)>,
        start: u32,
        count: u32,
    },
    Drawing {
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
    },
}

enum Encoding {
    Instance {
        punch: bool,
        picture: Option<(Image, bool)>,
        instance: Instance,
    },
    Drawing {
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
    },
}

#[derive(Clone, Copy)]
struct Walk {
    factor: f32,
    space: u32,
    origin: [f32; 2],
    bound: [f32; 4],
    offset: Vec2,
    clip: Rect,
}

impl Walk {
    fn key(&self, display: &Display) -> ListKey {
        ListKey {
            display: display.key,
            factor: self.factor.to_bits(),
            offset: [self.offset.x.to_bits(), self.offset.y.to_bits()],
            clip: [
                self.clip.min.x.to_bits(),
                self.clip.min.y.to_bits(),
                self.clip.max.x.to_bits(),
                self.clip.max.y.to_bits(),
            ],
        }
    }

    fn reach(&self, display: &Display) -> [f32; 4] {
        let visible = display.bounds.translate(self.offset).intersect(self.clip);
        let [left, top, right, bottom] = pixels(visible, self.factor, self.origin);
        overlap(
            [left - REACH, top - REACH, right + REACH, bottom + REACH],
            self.bound,
        )
    }

    fn enter(&self, entry: Entry, frame: &mut Frame) -> Self {
        let clip = self.clip.intersect(entry.clip.translate(self.offset));
        let Some(shift) = entry.shift else {
            return Self {
                offset: self.offset + entry.translation,
                clip,
                ..*self
            };
        };
        let at = self.offset + shift;
        let origin = [
            self.origin[0] + (at.x * self.factor).round(),
            self.origin[1] + (at.y * self.factor).round(),
        ];
        let bound = overlap(pixels(clip, self.factor, self.origin), self.bound);
        Self {
            factor: self.factor,
            space: frame.space(origin, bound),
            origin,
            bound,
            offset: entry.translation - shift,
            clip: Rect::EVERYTHING,
        }
    }
}

struct Frame<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    damaged: Option<&'a [[f32; 4]]>,
    screen: Vec2,
    pixels_per_point: f32,
    spaces: Vec<Space>,
    slots: HashMap<[u32; 6], u32>,
    staging: Vec<Instance>,
    drawings: Vec<(Drawing, DrawAt)>,
    overflow: bool,
}

impl Frame<'_> {
    fn space(&mut self, origin: [f32; 2], bound: [f32; 4]) -> u32 {
        let bound = bound.map(|value| value.clamp(OPEN[0], OPEN[2]));
        let key = [
            origin[0].to_bits(),
            origin[1].to_bits(),
            bound[0].to_bits(),
            bound[1].to_bits(),
            bound[2].to_bits(),
            bound[3].to_bits(),
        ];
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }
        let slot = self.spaces.len() as u32;
        self.spaces.push(Space {
            translation: origin,
            padding: [0.0; 2],
            clip: bound,
        });
        self.slots.insert(key, slot);
        slot
    }

    fn damages(&self, reach: [f32; 4]) -> bool {
        reach[0] < reach[2]
            && reach[1] < reach[3]
            && self.damaged.is_none_or(|damaged| {
                damaged
                    .iter()
                    .any(|damaged| overlaps(*damaged, reach))
            })
    }
}

fn pixels(rect: Rect, factor: f32, origin: [f32; 2]) -> [f32; 4] {
    [
        (rect.min.x * factor).round() + origin[0],
        (rect.min.y * factor).round() + origin[1],
        (rect.max.x * factor).round() + origin[0],
        (rect.max.y * factor).round() + origin[1],
    ]
}

fn overlap(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[0].max(right[0]),
        left[1].max(right[1]),
        left[2].min(right[2]),
        left[3].min(right[3]),
    ]
}

fn overlaps(left: [f32; 4], right: [f32; 4]) -> bool {
    let [x0, y0, x1, y1] = overlap(left, right);
    x0 < x1 && y0 < y1
}

fn moved(rect: [f32; 4], origin: [f32; 2]) -> [f32; 4] {
    [
        rect[0] + origin[0],
        rect[1] + origin[1],
        rect[2] + origin[0],
        rect[3] + origin[1],
    ]
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
        let space_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui space layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Space>() as u64),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout), Some(&space_layout)],
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
        let instance_buffer = vertex_buffer(device, "beui instances", instance_capacity);
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let space_stride = (std::mem::size_of::<Space>() as u64).div_ceil(alignment) * alignment;
        let (space_buffer, space_group) =
            space_buffer(device, &space_layout, space_stride, SPACES);
        let list_buffer = vertex_buffer(device, "beui listed instances", LISTED);
        let atlas = Atlas::new(device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui smooth sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui sharp sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &atlas.view,
            &sampler,
        );

        Self {
            srgb: format.is_srgb(),
            format,
            pipeline,
            punch_pipeline,
            bind_group_layout,
            bind_group,
            uniform_buffer,
            instance_buffer,
            instance_capacity,
            space_layout,
            space_buffer,
            space_group,
            space_capacity: SPACES,
            space_stride,
            list_buffer,
            list_capacity: LISTED,
            list_top: 0,
            generation: 0,
            lists: HashMap::new(),
            live: 0,
            atlas_epoch: 0,
            #[cfg(test)]
            encoded: 0,
            runs: Vec::new(),
            overlay: Vec::new(),
            scissors: None,
            origin: Vec2::ZERO,
            atlas,
            pictures: HashMap::new(),
            samplers: [nearest, sampler],
            filter: None,
            effects: None,
            bounds: None,
            whole: true,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &Image) {
        if let Some(picture) = self.pictures.get_mut(&image.id()) {
            picture.used = true;
            return;
        }
        let format = match self.srgb {
            true => wgpu::TextureFormat::Rgba8UnormSrgb,
            false => wgpu::TextureFormat::Rgba8Unorm,
        };
        let size = wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width() * 4),
                rows_per_image: Some(image.height()),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_groups = [
            bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &view,
                &self.samplers[0],
            ),
            bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &view,
                &self.samplers[1],
            ),
        ];
        self.pictures.insert(
            image.id(),
            Picture {
                bind_groups,
                used: true,
            },
        );
    }

    pub fn set_origin(&mut self, origin: Vec2) {
        self.origin = origin;
    }

    pub fn set_bounds(&mut self, bounds: Option<[u32; 4]>) {
        self.bounds = bounds;
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
            self.atlas_epoch += 1;
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                screen: [screen.x, screen.y],
                origin: [self.origin.x, self.origin.y],
            }),
        );

        let prepared = output
            .filter()
            .map(|filter: Filter| filter::Prepared::new(&filter, pixels_per_point, screen));
        let repaint = match prepared == self.filter {
            true => repaint,
            false => Repaint::Everything,
        };
        let damaged = match repaint {
            Repaint::Everything => None,
            Repaint::Region { region, .. } => {
                let regions: Vec<[f32; 4]> = match prepared.is_some() {
                    true => vec![physical(
                        region.bounds(),
                        self.origin,
                        screen,
                        pixels_per_point,
                    )],
                    false => region
                        .rects()
                        .iter()
                        .map(|rect| physical(*rect, self.origin, screen, pixels_per_point))
                        .collect(),
                };
                Some(match &prepared {
                    Some(prepared) => regions
                        .into_iter()
                        .map(|region| prepared.widen(region))
                        .collect(),
                    None => regions,
                })
            }
        };
        let (instances, runs, overlay, frame) = loop {
            let mut frame = Frame {
                device,
                queue,
                damaged: damaged.as_deref(),
                screen,
                pixels_per_point,
                spaces: Vec::new(),
                slots: HashMap::new(),
                staging: Vec::new(),
                drawings: Vec::new(),
                overflow: false,
            };
            frame.space([0.0; 2], OPEN);
            let top = self.list_top;
            let drawn = self.emit(&mut frame, output, repaint, prepared.is_some());
            if !frame.overflow {
                if !frame.staging.is_empty() {
                    queue.write_buffer(
                        &self.list_buffer,
                        (top * std::mem::size_of::<Instance>()) as u64,
                        bytemuck::cast_slice(&frame.staging),
                    );
                }
                break (drawn.0, drawn.1, drawn.2, frame);
            }
            self.generation += 1;
            self.list_top = 0;
            let needed = frame.staging.len().max(self.list_capacity);
            self.list_capacity = (needed * 2).next_power_of_two();
            self.list_buffer = vertex_buffer(device, "beui listed instances", self.list_capacity);
        };
        if frame.spaces.len() > self.space_capacity {
            self.space_capacity = frame.spaces.len().next_power_of_two();
            (self.space_buffer, self.space_group) = space_buffer(
                device,
                &self.space_layout,
                self.space_stride,
                self.space_capacity,
            );
        }
        let stride = self.space_stride as usize;
        let mut spaces = vec![0u8; stride * frame.spaces.len()];
        for (index, space) in frame.spaces.iter().enumerate() {
            let bytes = bytemuck::bytes_of(space);
            spaces[index * stride..index * stride + bytes.len()].copy_from_slice(bytes);
        }
        queue.write_buffer(&self.space_buffer, 0, &spaces);
        self.record_drawings(device, queue, &frame.drawings);
        if self.lists.len() > 2 * self.live + LISTED {
            self.sweep(output, pixels_per_point);
        }

        self.pictures.retain(|_, picture| {
            let used = picture.used;
            picture.used = false;
            used
        });
        let origin = self.origin;
        self.whole = damaged.is_none();
        let scissors = damaged.map(|damaged| {
            damaged
                .into_iter()
                .map(|damaged| scissor(damaged, origin))
                .collect::<Vec<_>>()
        });
        self.scissors = match (scissors, self.bounds) {
            (scissors, None) => scissors,
            (None, Some(bounds)) => Some(vec![bounds]),
            (Some(scissors), Some(bounds)) => Some(
                scissors
                    .into_iter()
                    .map(|scissor| intersected(scissor, bounds))
                    .collect(),
            ),
        };
        self.runs = runs;
        self.overlay = overlay;
        self.filter = prepared;
        if instances.is_empty() {
            return repaint;
        }
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = vertex_buffer(device, "beui instances", self.instance_capacity);
        }
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        repaint
    }

    fn emit(
        &mut self,
        frame: &mut Frame,
        output: &FrameOutput,
        repaint: Repaint,
        filtered: bool,
    ) -> (Vec<Instance>, Vec<Run>, Vec<Run>) {
        let mut instances = Vec::new();
        let mut runs = Vec::new();
        let mut overlay = Vec::new();
        if let (Repaint::Region { background, .. }, Some(damaged)) = (repaint, frame.damaged) {
            let [.., alpha] = background.to_array();
            for region in damaged {
                if alpha < u8::MAX {
                    Run::push(&mut runs, frame_run(true, None, instances.len()));
                    instances.push(Instance {
                        rect: *region,
                        clip: *region,
                        uv: [0.0; 4],
                        color: [0.0, 0.0, 0.0, 1.0],
                        params: [0.0, 0.0, 0.0, 0.0],
                        turn: turn(Turn::NONE),
                    });
                }
                if alpha == 0 {
                    continue;
                }
                Run::push(&mut runs, frame_run(false, None, instances.len()));
                instances.push(Instance {
                    rect: *region,
                    clip: *region,
                    uv: [0.0; 4],
                    color: self.encode(background),
                    params: [0.0, 0.0, 0.0, 0.0],
                    turn: turn(Turn::NONE),
                });
            }
        }
        let boundary = match filtered {
            true => output.filter.map(|(_, boundary)| boundary),
            false => None,
        };
        let pixels_per_point = frame.pixels_per_point;
        let mut quads = Vec::new();
        for (index, layer) in output.layers.iter().enumerate() {
            let target = match boundary.is_none_or(|boundary| index < boundary) {
                true => &mut runs,
                false => &mut overlay,
            };
            match layer {
                Layer::Shape(shape) => {
                    quads.clear();
                    push_quads(shape, pixels_per_point, frame.damaged, &mut quads);
                    for quad in quads.drain(..) {
                        match self.encoding(frame.queue, quad) {
                            Some(Encoding::Instance {
                                punch,
                                picture,
                                instance,
                            }) => {
                                if let Some((image, _)) = &picture {
                                    self.upload(frame.device, frame.queue, image);
                                }
                                let picture =
                                    picture.map(|(image, smooth)| (image.id(), smooth));
                                Run::push(target, frame_run(punch, picture, instances.len()));
                                instances.push(instance);
                            }
                            Some(Encoding::Drawing {
                                drawing,
                                rect,
                                clip,
                            }) => self.push_drawing(frame, target, drawing, rect, clip, OPEN),
                            None => {}
                        }
                    }
                }
                Layer::Display {
                    display,
                    entry,
                    scale,
                    clip,
                } => {
                    let bound = overlap(pixels(*clip, pixels_per_point, [0.0; 2]), OPEN);
                    let at = Walk {
                        factor: pixels_per_point * scale,
                        space: frame.space([0.0; 2], bound),
                        origin: [0.0; 2],
                        bound,
                        offset: entry.translation,
                        clip: entry.clip,
                    };
                    let mut top = Vec::new();
                    self.walk(frame, display, at, target, &mut top);
                    for run in top {
                        Run::push(target, run);
                    }
                }
            }
        }
        (instances, runs, overlay)
    }

    fn walk(
        &mut self,
        frame: &mut Frame,
        display: &Display,
        at: Walk,
        main: &mut Vec<Run>,
        top: &mut Vec<Run>,
    ) {
        if !frame.damages(at.reach(display)) {
            return;
        }
        let key = at.key(display);
        let mut encoded = match self.lists.remove(&key) {
            Some(encoded) if encoded.atlas == self.atlas_epoch => encoded,
            _ => self.encode_list(frame.queue, display, &at),
        };
        let base = match encoded.placed {
            Some((generation, start)) if generation == self.generation => start,
            _ => {
                let start = self.list_top;
                if start + encoded.instances.len() > self.list_capacity {
                    frame.overflow = true;
                }
                self.list_top += encoded.instances.len();
                frame.staging.extend_from_slice(&encoded.instances);
                encoded.placed = Some((self.generation, start as u32));
                start as u32
            }
        };
        for (part, (from, to)) in display.parts.iter().zip(&encoded.parts) {
            match part {
                Part::Shapes { top: on_top, .. } => {
                    let target = match on_top {
                        true => &mut *top,
                        false => &mut *main,
                    };
                    for run in &encoded.runs[*from..*to] {
                        match run {
                            ListRun::Quads {
                                punch,
                                picture,
                                start,
                                count,
                            } => {
                                if let Some((image, _)) = picture {
                                    self.upload(frame.device, frame.queue, image);
                                }
                                Run::push(
                                    target,
                                    Run {
                                        punch: *punch,
                                        picture: picture
                                            .as_ref()
                                            .map(|(image, smooth)| (image.id(), *smooth)),
                                        drawing: None,
                                        listed: true,
                                        space: at.space,
                                        start: base + start,
                                        count: *count,
                                    },
                                );
                            }
                            ListRun::Drawing {
                                drawing,
                                rect,
                                clip,
                            } => self.push_drawing(
                                frame,
                                target,
                                drawing.clone(),
                                moved(*rect, at.origin),
                                moved(*clip, at.origin),
                                at.bound,
                            ),
                        }
                    }
                }
                Part::Child(_, entry, child) => {
                    let inner = at.enter(*entry, frame);
                    self.walk(frame, child, inner, main, top);
                }
            }
        }
        self.lists.insert(key, encoded);
    }

    fn push_drawing(
        &self,
        frame: &mut Frame,
        target: &mut Vec<Run>,
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
        bound: [f32; 4],
    ) {
        let clip = overlap(clip, bound);
        if !frame.damages(overlap(rect, clip)) {
            return;
        }
        let shift = |[left, top, right, bottom]: [f32; 4]| {
            [
                left - self.origin.x,
                top - self.origin.y,
                right - self.origin.x,
                bottom - self.origin.y,
            ]
        };
        let at = DrawAt {
            rect: shift(rect),
            clip: shift(clip),
            screen: frame.screen,
            pixels_per_point: frame.pixels_per_point,
            format: self.format,
        };
        frame.drawings.push((drawing.clone(), at));
        target.push(Run {
            punch: false,
            picture: None,
            drawing: Some((drawing, at)),
            listed: false,
            space: 0,
            start: 0,
            count: 0,
        });
    }

    fn encode_list(&mut self, queue: &wgpu::Queue, display: &Display, at: &Walk) -> Encoded {
        let mut instances = Vec::new();
        let mut runs: Vec<ListRun> = Vec::new();
        let mut parts = Vec::with_capacity(display.parts.len());
        let mut quads = Vec::new();
        for part in display.parts.iter() {
            let Part::Shapes { start, end, .. } = part else {
                parts.push((runs.len(), runs.len()));
                continue;
            };
            let from = runs.len();
            for shape in &display.shapes[*start as usize..*end as usize] {
                let shape = placed_shape(shape, at.offset, at.clip);
                quads.clear();
                push_quads(&shape, at.factor, None, &mut quads);
                for quad in quads.drain(..) {
                    match self.encoding(queue, quad) {
                        Some(Encoding::Instance {
                            punch,
                            picture,
                            instance,
                        }) => {
                            let at = instances.len() as u32;
                            instances.push(instance);
                            if let Some(ListRun::Quads {
                                punch: held,
                                picture: pictured,
                                count,
                                ..
                            }) = runs[from..].last_mut()
                                && *held == punch
                                && pictured.as_ref().map(|(image, smooth)| (image.id(), *smooth))
                                    == picture.as_ref().map(|(image, smooth)| (image.id(), *smooth))
                            {
                                *count += 1;
                                continue;
                            }
                            runs.push(ListRun::Quads {
                                punch,
                                picture,
                                start: at,
                                count: 1,
                            });
                        }
                        Some(Encoding::Drawing {
                            drawing,
                            rect,
                            clip,
                        }) => runs.push(ListRun::Drawing {
                            drawing,
                            rect,
                            clip,
                        }),
                        None => {}
                    }
                }
            }
            parts.push((from, runs.len()));
        }
        #[cfg(test)]
        {
            self.encoded += instances.len();
        }
        Encoded {
            instances,
            runs,
            parts,
            placed: None,
            atlas: self.atlas_epoch,
        }
    }

    fn sweep(&mut self, output: &FrameOutput, pixels_per_point: f32) {
        let mut live = std::collections::HashSet::new();
        for layer in output.layers.iter() {
            let Layer::Display {
                display,
                entry,
                scale,
                clip,
            } = layer
            else {
                continue;
            };
            let bound = overlap(pixels(*clip, pixels_per_point, [0.0; 2]), OPEN);
            let at = Walk {
                factor: pixels_per_point * scale,
                space: 0,
                origin: [0.0; 2],
                bound,
                offset: entry.translation,
                clip: entry.clip,
            };
            mark(display, at, &mut live);
        }
        self.lists.retain(|key, _| live.contains(key));
        self.live = live.len();
    }

    fn encoding(&mut self, queue: &wgpu::Queue, quad: Quad) -> Option<Encoding> {
        let plain = |instance| Encoding::Instance {
            punch: false,
            picture: None,
            instance,
        };
        Some(match quad {
            Quad::Rect {
                rect,
                clip,
                color,
                corner_radius,
                stroke_width,
                turn: rotation,
            } => plain(Instance {
                rect,
                clip,
                uv: [0.0; 4],
                color: self.encode(color),
                params: [corner_radius, stroke_width, 0.0, 0.0],
                turn: turn(rotation),
            }),
            Quad::Glyph {
                rect,
                clip,
                color,
                glyph,
                turn: rotation,
            } => {
                let uv = self.atlas.insert(queue, glyph.id, &glyph.image)?;
                plain(Instance {
                    rect,
                    clip,
                    uv,
                    color: self.encode(color),
                    params: [0.0, 0.0, 1.0, 0.0],
                    turn: turn(rotation),
                })
            }
            Quad::Image {
                rect,
                clip,
                source,
                image,
                tint,
                corner_radius,
                smooth,
                turn: rotation,
            } => Encoding::Instance {
                punch: false,
                instance: Instance {
                    rect,
                    clip,
                    uv: source,
                    color: self.encode(tint),
                    params: [corner_radius, 0.0, 2.0, 0.0],
                    turn: turn(rotation),
                },
                picture: Some((image, smooth)),
            },
            Quad::Line {
                rect,
                clip,
                segment,
                width,
                color,
            } => plain(Instance {
                rect,
                clip,
                uv: segment,
                color: self.encode(color),
                params: [width / 2.0, 0.0, 3.0, 0.0],
                turn: turn(Turn::NONE),
            }),
            Quad::Punch {
                rect,
                clip,
                corner_radius,
                turn: rotation,
            } => Encoding::Instance {
                punch: true,
                picture: None,
                instance: Instance {
                    rect,
                    clip,
                    uv: [0.0; 4],
                    color: [0.0, 0.0, 0.0, 1.0],
                    params: [corner_radius, 0.0, 0.0, 0.0],
                    turn: turn(rotation),
                },
            },
            Quad::Drawing {
                rect,
                clip,
                drawing,
            } => Encoding::Drawing {
                drawing,
                rect,
                clip,
            },
        })
    }

    fn record_drawings(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawings: &[(Drawing, DrawAt)],
    ) {
        if drawings.is_empty() {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("beui drawings"),
        });
        for (drawing, at) in drawings {
            if let Some(draw) = drawing.draw() {
                draw.prepare(device, queue, &mut encoder, *at);
            }
        }
        queue.submit([encoder.finish()]);
    }

    fn encode(&self, color: Color32) -> [f32; 4] {
        match self.srgb {
            true => color.to_linear_f32(),
            false => color.to_normalized_gamma_f32(),
        }
    }

    pub fn scissors(&self) -> Option<&[[u32; 4]]> {
        self.scissors.as_deref()
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(scissors) = &self.scissors else {
            self.draw(pass, &self.runs, None);
            self.draw(pass, &self.overlay, None);
            return;
        };
        for scissor in scissors {
            if clip(pass, *scissor) {
                self.draw(pass, &self.runs, Some(*scissor));
                self.draw(pass, &self.overlay, Some(*scissor));
            }
        }
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
        let scissor = self
            .scissors
            .as_ref()
            .and_then(|scissors| scissors.first().copied());
        let (format, srgb) = (self.format, self.srgb);
        self.effects
            .get_or_insert_with(|| filter::Effects::new(device, format))
            .ensure(device, size);
        {
            let Some(scene) = self.effects.as_ref().and_then(filter::Effects::scene) else {
                return;
            };
            let scene_load = match load {
                wgpu::LoadOp::Load if self.whole => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                load => load,
            };
            let mut pass = self.begin(encoder, scene, scene_load);
            if let Some(scissor) = scissor {
                clip(&mut pass, scissor);
            }
            self.draw(&mut pass, &self.runs, scissor);
        }
        if let Some(effects) = self.effects.as_mut() {
            effects.record(device, queue, encoder, &prepared, srgb, scissor);
        }
        let mut pass = self.begin(encoder, target, load);
        if let Some(scissor) = scissor {
            clip(&mut pass, scissor);
        }
        if let Some(effects) = self.effects.as_ref() {
            effects.compose(&mut pass);
        }
        self.draw(&mut pass, &self.overlay, scissor);
    }

    fn empty(&self) -> bool {
        self.scissors.as_ref().is_some_and(|scissors| {
            scissors
                .iter()
                .all(|[_, _, width, height]| *width == 0 || *height == 0)
        })
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

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, runs: &[Run], scissor: Option<[u32; 4]>) {
        if runs.is_empty() {
            return;
        }
        let mut bound = None;
        let mut buffer = None;
        let mut space = None;
        for run in runs {
            if let Some((drawing, at)) = run.drawing.as_ref() {
                if let (Some(draw), Some(at)) = (drawing.draw(), within(*at, scissor)) {
                    let bounds = scissor.unwrap_or([0, 0, at.screen.x as u32, at.screen.y as u32]);
                    if clip(pass, clipped(at.clip, bounds)) {
                        draw.paint(pass, at);
                    }
                    clip(pass, bounds);
                    bound = None;
                    buffer = None;
                    space = None;
                }
                continue;
            }
            let group = run.picture.and_then(|(id, smooth)| {
                self.pictures
                    .get(&id)
                    .map(|picture| &picture.bind_groups[usize::from(smooth)])
            });
            if run.picture.is_some() && group.is_none() {
                continue;
            }
            if buffer != Some(run.listed) {
                let instances = match run.listed {
                    true => &self.list_buffer,
                    false => &self.instance_buffer,
                };
                pass.set_vertex_buffer(0, instances.slice(..));
                buffer = Some(run.listed);
            }
            if bound != Some(run.picture) {
                pass.set_bind_group(0, group.unwrap_or(&self.bind_group), &[]);
                bound = Some(run.picture);
            }
            if space != Some(run.space) {
                let offset = (u64::from(run.space) * self.space_stride) as u32;
                pass.set_bind_group(1, &self.space_group, &[offset]);
                space = Some(run.space);
            }
            pass.set_pipeline(match run.punch {
                true => &self.punch_pipeline,
                false => &self.pipeline,
            });
            pass.draw(0..6, run.start..run.start + run.count);
        }
    }
}

fn frame_run(punch: bool, picture: Option<(ImageId, bool)>, at: usize) -> Run {
    Run {
        punch,
        picture,
        drawing: None,
        listed: false,
        space: 0,
        start: at as u32,
        count: 1,
    }
}

fn mark(display: &Display, at: Walk, live: &mut std::collections::HashSet<ListKey>) {
    let reach = at.reach(display);
    if reach[0] >= reach[2] || reach[1] >= reach[3] {
        return;
    }
    live.insert(at.key(display));
    for (_, entry, child) in display.children() {
        let inner = match entry.shift {
            None => Walk {
                offset: at.offset + entry.translation,
                clip: at.clip.intersect(entry.clip.translate(at.offset)),
                ..at
            },
            Some(shift) => {
                let clip = at.clip.intersect(entry.clip.translate(at.offset));
                let origin = [
                    at.origin[0] + ((at.offset.x + shift.x) * at.factor).round(),
                    at.origin[1] + ((at.offset.y + shift.y) * at.factor).round(),
                ];
                Walk {
                    factor: at.factor,
                    space: 0,
                    origin,
                    bound: overlap(pixels(clip, at.factor, at.origin), at.bound),
                    offset: entry.translation - shift,
                    clip: Rect::EVERYTHING,
                }
            }
        };
        mark(child, inner, live);
    }
}

fn turn(turn: Turn) -> [f32; 4] {
    [turn.pivot[0], turn.pivot[1], turn.cos, turn.sin]
}

fn physical(region: Rect, origin: Vec2, screen: Vec2, pixels_per_point: f32) -> [f32; 4] {
    [
        (region.left() * pixels_per_point)
            .floor()
            .clamp(origin.x, origin.x + screen.x),
        (region.top() * pixels_per_point)
            .floor()
            .clamp(origin.y, origin.y + screen.y),
        (region.right() * pixels_per_point)
            .ceil()
            .clamp(origin.x, origin.x + screen.x),
        (region.bottom() * pixels_per_point)
            .ceil()
            .clamp(origin.y, origin.y + screen.y),
    ]
}

fn clip(pass: &mut wgpu::RenderPass<'_>, [left, top, width, height]: [u32; 4]) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    pass.set_scissor_rect(left, top, width, height);
    true
}

fn within(at: DrawAt, scissor: Option<[u32; 4]>) -> Option<DrawAt> {
    let Some([left, top, width, height]) = scissor else {
        return Some(at);
    };
    let [left, top] = [left as f32, top as f32];
    let clip = [
        at.clip[0].max(left),
        at.clip[1].max(top),
        at.clip[2].min(left + width as f32),
        at.clip[3].min(top + height as f32),
    ];
    (clip[0] < clip[2] && clip[1] < clip[3]).then_some(DrawAt { clip, ..at })
}

fn clipped(clip: [f32; 4], [left, top, width, height]: [u32; 4]) -> [u32; 4] {
    let (right, bottom) = (left + width, top + height);
    let from = |value: f32, low: u32, high: u32| value.clamp(low as f32, high as f32);
    let (x0, y0) = (
        from(clip[0].floor(), left, right),
        from(clip[1].floor(), top, bottom),
    );
    let (x1, y1) = (
        from(clip[2].ceil(), left, right),
        from(clip[3].ceil(), top, bottom),
    );
    [
        x0 as u32,
        y0 as u32,
        (x1 - x0).max(0.0) as u32,
        (y1 - y0).max(0.0) as u32,
    ]
}

fn intersected([left, top, width, height]: [u32; 4], bounds: [u32; 4]) -> [u32; 4] {
    let [bound_left, bound_top, bound_width, bound_height] = bounds;
    let x0 = left.max(bound_left);
    let y0 = top.max(bound_top);
    let x1 = (left + width).min(bound_left + bound_width);
    let y1 = (top + height).min(bound_top + bound_height);
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

fn scissor(damaged: [f32; 4], origin: Vec2) -> [u32; 4] {
    let width = (damaged[2] - damaged[0]).max(0.0) as u32;
    let height = (damaged[3] - damaged[1]).max(0.0) as u32;
    [
        (damaged[0] - origin.x) as u32,
        (damaged[1] - origin.y) as u32,
        width,
        height,
    ]
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

fn vertex_buffer(device: &wgpu::Device, label: &str, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn space_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("beui spaces"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beui space group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<Space>() as u64),
            }),
        }],
    });
    (buffer, group)
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
    view: &wgpu::TextureView,
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
                resource: wgpu::BindingResource::TextureView(view),
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
