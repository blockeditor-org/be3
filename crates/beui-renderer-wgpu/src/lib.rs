#[cfg(test)]
extern crate beui_view as beui;

use std::collections::HashMap;
use std::rc::Rc;

use bytemuck::{Pod, Zeroable};

use beui_core::color::Color32;
use beui_core::context::{Moved, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::{Display, Group, Layer, Part};
use beui_core::draw::{Quad, Turn, push_quads};
pub use beui_core::drawing::Drawing;
use beui_core::fade::Fade;
use beui_core::filter::Filter;
use beui_core::font::{GlyphId, GlyphImage};
use beui_core::geometry::{Rect, Vec2};
use beui_core::image::{Image, ImageId};
use beui_core::painter::placed_shape;

pub mod filter;
pub mod present;

#[cfg(all(target_arch = "wasm32", feature = "web"))]
pub mod canvas;
#[cfg(not(target_arch = "wasm32"))]
pub mod offscreen;
#[cfg(not(target_arch = "wasm32"))]
pub mod window;

use beui_core::context::RendererInfo;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawAt {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub screen: Vec2,
    pub pixels_per_point: f32,
    pub format: wgpu::TextureFormat,
    pub renderer: u64,
}

static RENDERERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DrawAt {
    pub fn width(&self) -> u32 {
        (self.rect[2] - self.rect[0]).max(0.0) as u32
    }

    pub fn height(&self) -> u32 {
        (self.rect[3] - self.rect[1]).max(0.0) as u32
    }
}

pub trait Draw: 'static {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _at: DrawAt,
    ) {
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt);
}

pub fn drawing(draw: impl Draw) -> Drawing {
    let draw: Rc<dyn Draw> = Rc::new(draw);
    Drawing::erased(draw)
}

pub fn painted_drawing(draw: impl Draw + beui_core::painting::Paints) -> Drawing {
    let draw = Rc::new(draw);
    let paints: Rc<dyn beui_core::painting::Paints> = draw.clone();
    let draw: Rc<dyn Draw> = draw;
    Drawing::painted(draw, paints)
}

pub fn renderer_info(adapter: &wgpu::AdapterInfo, format: wgpu::TextureFormat) -> RendererInfo {
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
    rows.push(("Surface format", format!("{format:?}")));
    RendererInfo { rows }
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
    texel: u32,
    view: wgpu::TextureView,
    entries: HashMap<GlyphId, [f32; 4]>,
    row_y: u32,
    row_height: u32,
    cursor_x: u32,
    full: bool,
}

impl Atlas {
    fn new(device: &wgpu::Device, label: &str, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texel: format.block_copy_size(None).unwrap_or(1),
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
        let row = image.width * self.texel;
        if image.pixels.len() < row as usize * image.height as usize {
            return None;
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
                bytes_per_row: Some(row),
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

pub trait Repainting {
    fn repaint(&self, background: Color32) -> Repaint;

    fn repaint_moving(&self, background: Color32) -> (Repaint, Option<Moved>);
}

impl Repainting for WindowOutput {
    fn repaint(&self, background: Color32) -> Repaint {
        let region = self.unmoved();
        match region.is_empty() {
            true => Repaint::Everything,
            false => Repaint::Region { region, background },
        }
    }

    fn repaint_moving(&self, background: Color32) -> (Repaint, Option<Moved>) {
        match (self.moved, self.damage.is_empty()) {
            (None, _) | (Some(_), true) => (self.repaint(background), None),
            (Some(moved), false) => (
                Repaint::Region {
                    region: self.damage,
                    background,
                },
                Some(moved),
            ),
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
    opacity: f32,
    padding: f32,
    clip: [f32; 4],
    fade: [f32; 4],
    widths: [f32; 4],
}

const OPEN: [f32; 4] = [-1.0e9, -1.0e9, 1.0e9, 1.0e9];
const SPACES: usize = 64;
const LISTED: usize = 4096;
const REACH: f32 = 2.0;

pub struct Renderer {
    id: u64,
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
    drawn: std::collections::HashSet<ListKey>,
    frames: u64,
    live: usize,
    atlas_epoch: u64,
    pub encoded: usize,
    steps: Vec<Step>,
    scissors: Option<Vec<[u32; 4]>>,
    origin: Vec2,
    atlas: Atlas,
    colors: Atlas,
    pictures: HashMap<ImageId, Picture>,
    samplers: [wgpu::Sampler; 2],
    filters: Vec<filter::Prepared>,
    effects: Vec<filter::Effects>,
    bounds: Option<[u32; 4]>,
    whole: bool,
    scratch: Option<wgpu::Texture>,
}

#[derive(Clone)]
struct Run {
    punch: bool,
    picture: Option<(ImageId, bool)>,
    drawing: Option<(Drawing, DrawAt)>,
    listed: bool,
    space: u32,
    start: u32,
    count: u32,
}

struct Emitted {
    instances: Vec<Instance>,
    background: Vec<Run>,
    cleared: Vec<Run>,
    filters: usize,
}

enum Step {
    Runs(Vec<Run>),
    Filter {
        index: usize,
        bottom: bool,
        steps: Vec<Step>,
    },
}

fn push_run(steps: &mut Vec<Step>, run: Run) {
    match steps.last_mut() {
        Some(Step::Runs(runs)) => Run::push(runs, run),
        _ => steps.push(Step::Runs(vec![run])),
    }
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
    seen: u64,
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
    fade: Fade,
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

    fn child(&self, at: Vec2) -> Self {
        Self {
            offset: self.offset + at,
            ..*self
        }
    }

    fn group(
        &self,
        group: &Group,
        space: &mut impl FnMut([f32; 2], [f32; 4], Fade) -> u32,
    ) -> Self {
        match *group {
            Group::Clip(clip) => Self {
                clip: self.clip.intersect(clip.translate(self.offset)),
                ..*self
            },
            Group::Scroll {
                viewport,
                offset,
                scale,
                ..
            } => {
                let clip = self.clip.intersect(viewport.translate(self.offset));
                let at = self.offset + offset;
                let origin = [
                    self.origin[0] + (at.x * self.factor).round(),
                    self.origin[1] + (at.y * self.factor).round(),
                ];
                let bound = overlap(pixels(clip, self.factor, self.origin), self.bound);
                let scale = match scale > 0.0 && scale.is_finite() {
                    true => scale,
                    false => 1.0,
                };
                Self {
                    factor: self.factor * scale,
                    space: space(origin, bound, self.fade),
                    origin,
                    bound,
                    offset: Vec2::ZERO,
                    clip: Rect::EVERYTHING,
                    fade: self.fade,
                }
            }
            Group::Mask { fade, .. } => {
                let fade = self.fade.within(
                    fade.translate(self.offset)
                        .scaled(self.factor, Vec2::new(self.origin[0], self.origin[1])),
                );
                Self {
                    space: space(self.origin, self.bound, fade),
                    fade,
                    ..*self
                }
            }
            Group::Transform { offset, scale } if scale > 0.0 && scale.is_finite() => Self {
                factor: self.factor * scale,
                offset: (self.offset + offset) * scale.recip(),
                clip: self.clip.scaled(scale.recip()),
                ..*self
            },
            Group::Transform { .. } | Group::Filter(_) => *self,
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
    slots: HashMap<[u32; 15], u32>,
    staging: Vec<Instance>,
    drawings: Vec<(Drawing, DrawAt)>,
    overflow: bool,
}

impl Frame<'_> {
    fn space(&mut self, origin: [f32; 2], bound: [f32; 4], fade: Fade) -> u32 {
        let bound = bound.map(|value| value.clamp(OPEN[0], OPEN[2]));
        let rect = [
            fade.rect.min.x,
            fade.rect.min.y,
            fade.rect.max.x,
            fade.rect.max.y,
        ]
        .map(|value| value.clamp(OPEN[0], OPEN[2]));
        let mut key = [0; 15];
        for (slot, value) in key.iter_mut().zip(
            origin
                .iter()
                .chain(&bound)
                .chain(&rect)
                .chain(&fade.widths)
                .chain([&fade.opacity]),
        ) {
            *slot = value.to_bits();
        }
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }
        let slot = self.spaces.len() as u32;
        self.spaces.push(Space {
            translation: origin,
            opacity: fade.opacity,
            padding: 0.0,
            clip: bound,
            fade: rect,
            widths: fade.widths,
        });
        self.slots.insert(key, slot);
        slot
    }

    fn damages(&self, reach: [f32; 4]) -> bool {
        reach[0] < reach[2]
            && reach[1] < reach[3]
            && self
                .damaged
                .is_none_or(|damaged| damaged.iter().any(|damaged| overlaps(*damaged, reach)))
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

fn at(texture: &wgpu::Texture, x: f32, y: f32) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d {
            x: x as u32,
            y: y as u32,
            z: 0,
        },
        aspect: wgpu::TextureAspect::All,
    }
}

fn translated(rect: [f32; 4], origin: [f32; 2]) -> [f32; 4] {
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let space_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui space layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let (space_buffer, space_group) = space_buffer(device, &space_layout, space_stride, SPACES);
        let list_buffer = vertex_buffer(device, "beui listed instances", LISTED);
        let atlas = Atlas::new(device, "beui glyph atlas", wgpu::TextureFormat::R8Unorm);
        let colors = Atlas::new(device, "beui colour glyph atlas", rgba(format.is_srgb()));
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
            &colors.view,
        );

        Self {
            id: RENDERERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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
            drawn: std::collections::HashSet::new(),
            frames: 0,
            live: 0,
            atlas_epoch: 0,
            encoded: 0,
            steps: Vec::new(),
            scissors: None,
            origin: Vec2::ZERO,
            atlas,
            colors,
            pictures: HashMap::new(),
            samplers: [nearest, sampler],
            filters: Vec::new(),
            effects: Vec::new(),
            bounds: None,
            whole: true,
            scratch: None,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &Image) {
        if let Some(picture) = self.pictures.get_mut(&image.id()) {
            picture.used = true;
            return;
        }
        let format = rgba(self.srgb);
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
                &self.colors.view,
            ),
            bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &view,
                &self.samplers[1],
                &self.colors.view,
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

    pub fn shift(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        moved: Moved,
        pixels_per_point: f32,
    ) {
        let size = texture.size();
        let whole = [0.0, 0.0, size.width as f32, size.height as f32];
        let by = [
            (moved.by.x * pixels_per_point).round(),
            (moved.by.y * pixels_per_point).round(),
        ];
        let mut from = overlap(
            translated(
                pixels(moved.from, pixels_per_point, [0.0; 2]),
                [-self.origin.x, -self.origin.y],
            ),
            whole,
        );
        from = overlap(from, translated(whole, [-by[0], -by[1]]));
        if let Some([left, top, width, height]) = self.bounds {
            let bounds = [
                left as f32,
                top as f32,
                (left + width) as f32,
                (top + height) as f32,
            ];
            from = overlap(overlap(from, bounds), translated(bounds, [-by[0], -by[1]]));
        }
        let [left, top, right, bottom] = from;
        if left >= right || top >= bottom {
            return;
        }
        let extent = wgpu::Extent3d {
            width: (right - left) as u32,
            height: (bottom - top) as u32,
            depth_or_array_layers: 1,
        };
        let scratch = match &self.scratch {
            Some(scratch)
                if scratch.format() == texture.format()
                    && scratch.width() >= extent.width
                    && scratch.height() >= extent.height =>
            {
                scratch
            }
            _ => self
                .scratch
                .insert(device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("beui moved region"),
                    size: wgpu::Extent3d {
                        width: size.width,
                        height: size.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: texture.format(),
                    usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })),
        };
        encoder.copy_texture_to_texture(at(texture, left, top), at(scratch, 0.0, 0.0), extent);
        encoder.copy_texture_to_texture(
            at(scratch, 0.0, 0.0),
            at(texture, left + by[0], top + by[1]),
            extent,
        );
    }

    pub fn id(&self) -> u64 {
        self.id
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
        output: &WindowOutput,
        screen: Vec2,
        pixels_per_point: f32,
        repaint: Repaint,
    ) -> Repaint {
        if self.atlas.full || self.colors.full {
            self.atlas.reset();
            self.colors.reset();
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

        let prepared: Vec<filter::Prepared> = output
            .filters()
            .iter()
            .map(|filter: &Filter| filter::Prepared::new(filter, pixels_per_point, screen))
            .collect();
        let mut restyled = Vec::new();
        for index in 0..prepared.len().max(self.filters.len()) {
            let (now, was) = (prepared.get(index), self.filters.get(index));
            if now != was {
                let origin = [self.origin.x, self.origin.y];
                restyled.extend(
                    [now, was]
                        .into_iter()
                        .flatten()
                        .map(|filter| translated(filter.region(), origin)),
                );
            }
        }
        let damaged: Option<Vec<[f32; 4]>> = match repaint {
            Repaint::Everything => None,
            Repaint::Region { region, .. } => {
                let regions = region
                    .rects()
                    .iter()
                    .map(|rect| physical(*rect, self.origin, screen, pixels_per_point))
                    .chain(restyled);
                Some(
                    regions
                        .map(|region: [f32; 4]| {
                            prepared
                                .iter()
                                .fold(region, |region, prepared| prepared.widen(region))
                        })
                        .collect(),
                )
            }
        };
        let (instances, steps, frame) = loop {
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
            frame.space([0.0; 2], OPEN, Fade::NONE);
            let top = self.list_top;
            let drawn = self.emit(&mut frame, output, repaint);
            if !frame.overflow {
                if !frame.staging.is_empty() {
                    queue.write_buffer(
                        &self.list_buffer,
                        (top * std::mem::size_of::<Instance>()) as u64,
                        bytemuck::cast_slice(&frame.staging),
                    );
                }
                break (drawn.0, drawn.1, frame);
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
        self.release_drawings();
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
        self.steps = steps;
        self.filters = prepared;
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
        output: &WindowOutput,
        repaint: Repaint,
    ) -> (Vec<Instance>, Vec<Step>) {
        let mut instances = Vec::new();
        let mut runs = Vec::new();
        if let (Repaint::Region { background, .. }, Some(damaged)) = (repaint, frame.damaged) {
            let [.., alpha] = background.to_array();
            for region in damaged {
                if alpha < u8::MAX {
                    Run::push(&mut runs, frame_run(true, None, 0, instances.len()));
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
                Run::push(&mut runs, frame_run(false, None, 0, instances.len()));
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
        let mut cleared = Vec::new();
        for region in frame.damaged.unwrap_or_default() {
            Run::push(&mut cleared, frame_run(true, None, 0, instances.len()));
            instances.push(Instance {
                rect: *region,
                clip: *region,
                uv: [0.0; 4],
                color: [0.0, 0.0, 0.0, 1.0],
                params: [0.0, 0.0, 0.0, 0.0],
                turn: turn(Turn::NONE),
            });
        }
        let mut steps = vec![Step::Runs(runs.clone())];
        let at = Walk {
            factor: frame.pixels_per_point,
            space: 0,
            origin: [0.0; 2],
            bound: OPEN,
            offset: Vec2::ZERO,
            clip: Rect::EVERYTHING,
            fade: Fade::NONE,
        };
        let mut emitted = Emitted {
            instances,
            background: runs,
            cleared,
            filters: 0,
        };
        self.emit_layers(frame, &output.layers, at, &mut emitted, &mut steps, true);
        (emitted.instances, steps)
    }

    fn emit_layers(
        &mut self,
        frame: &mut Frame,
        layers: &[Layer],
        at: Walk,
        emitted: &mut Emitted,
        steps: &mut Vec<Step>,
        bottom: bool,
    ) {
        let mut quads = Vec::new();
        for (index, layer) in layers.iter().enumerate() {
            let bottom = bottom && index == 0;
            match layer {
                Layer::Shape(shape) => {
                    let shape = placed_shape(shape, at.offset, at.clip);
                    quads.clear();
                    push_quads(&shape, at.factor, frame.damaged, &mut quads);
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
                                let picture = picture.map(|(image, smooth)| (image.id(), smooth));
                                push_run(
                                    steps,
                                    frame_run(punch, picture, at.space, emitted.instances.len()),
                                );
                                emitted.instances.push(instance);
                            }
                            Some(Encoding::Drawing {
                                drawing,
                                rect,
                                clip,
                            }) => {
                                if let Some(run) = self.push_drawing(
                                    frame,
                                    drawing,
                                    translated(rect, at.origin),
                                    translated(clip, at.origin),
                                    at.bound,
                                ) {
                                    push_run(steps, run);
                                }
                            }
                            None => {}
                        }
                    }
                }
                Layer::Display(translation, display) => {
                    self.walk(frame, display, at.child(*translation), steps);
                }
                Layer::Group(Group::Filter(_), inner) => {
                    let index = emitted.filters;
                    emitted.filters += 1;
                    let beneath = match bottom {
                        true => emitted.background.clone(),
                        false => emitted.cleared.clone(),
                    };
                    let mut filtered = vec![Step::Runs(beneath)];
                    self.emit_layers(frame, inner, at, emitted, &mut filtered, bottom);
                    steps.push(Step::Filter {
                        index,
                        bottom,
                        steps: filtered,
                    });
                }
                Layer::Group(group, inner) => {
                    let within = at.group(group, &mut |origin, bound, fade| {
                        frame.space(origin, bound, fade)
                    });
                    self.emit_layers(frame, inner, within, emitted, steps, false);
                }
            }
        }
    }

    fn walk(&mut self, frame: &mut Frame, display: &Display, at: Walk, steps: &mut Vec<Step>) {
        if !frame.damages(at.reach(display)) {
            return;
        }
        let key = at.key(display);
        let mut encoded = match self.lists.remove(&key) {
            Some(encoded) if encoded.atlas == self.atlas_epoch => encoded,
            _ => self.encode_list(frame.queue, display, &at),
        };
        encoded.seen = self.frames;
        if encoded
            .runs
            .iter()
            .any(|run| matches!(run, ListRun::Drawing { .. }))
        {
            self.drawn.insert(key);
        }
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
        let mut shapes = 0;
        self.walk_parts(
            frame,
            &display.parts,
            at,
            (&encoded, base),
            &mut shapes,
            steps,
        );
        self.lists.insert(key, encoded);
    }

    fn walk_parts(
        &mut self,
        frame: &mut Frame,
        parts: &[Part],
        at: Walk,
        (encoded, base): (&Encoded, u32),
        shapes: &mut usize,
        steps: &mut Vec<Step>,
    ) {
        for part in parts {
            match part {
                Part::Shapes { .. } => {
                    let (from, to) = encoded.parts[*shapes];
                    *shapes += 1;
                    for run in &encoded.runs[from..to] {
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
                                push_run(
                                    steps,
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
                            } => {
                                if let Some(run) = self.push_drawing(
                                    frame,
                                    drawing.clone(),
                                    translated(*rect, at.origin),
                                    translated(*clip, at.origin),
                                    at.bound,
                                ) {
                                    push_run(steps, run);
                                }
                            }
                        }
                    }
                }
                Part::Child(_, translation, child) => {
                    self.walk(frame, child, at.child(*translation), steps);
                }
                Part::Group(group, inner) => {
                    let within = at.group(group, &mut |origin, bound, fade| {
                        frame.space(origin, bound, fade)
                    });
                    self.walk_parts(frame, inner, within, (encoded, base), shapes, steps);
                }
            }
        }
    }

    fn push_drawing(
        &self,
        frame: &mut Frame,
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
        bound: [f32; 4],
    ) -> Option<Run> {
        let clip = overlap(clip, bound);
        if !frame.damages(overlap(rect, clip)) {
            return None;
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
            renderer: self.id,
        };
        frame.drawings.push((drawing.clone(), at));
        Some(Run {
            punch: false,
            picture: None,
            drawing: Some((drawing, at)),
            listed: false,
            space: 0,
            start: 0,
            count: 0,
        })
    }

    fn encode_list(&mut self, queue: &wgpu::Queue, display: &Display, at: &Walk) -> Encoded {
        let mut encoded = Encoded {
            instances: Vec::new(),
            runs: Vec::new(),
            parts: Vec::new(),
            placed: None,
            atlas: self.atlas_epoch,
            seen: self.frames,
        };
        self.encode_parts(queue, display, &display.parts, *at, &mut encoded);
        self.encoded += encoded.instances.len();
        encoded
    }

    fn encode_parts(
        &mut self,
        queue: &wgpu::Queue,
        display: &Display,
        parts: &[Part],
        at: Walk,
        encoded: &mut Encoded,
    ) {
        let mut quads = Vec::new();
        for part in parts {
            match part {
                Part::Child(..) => {}
                Part::Group(group, inner) => {
                    let within = at.group(group, &mut |_, _, _| 0);
                    self.encode_parts(queue, display, inner, within, encoded);
                }
                Part::Shapes { start, end, .. } => {
                    let from = encoded.runs.len();
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
                                    let index = encoded.instances.len() as u32;
                                    encoded.instances.push(instance);
                                    if let Some(ListRun::Quads {
                                        punch: held,
                                        picture: pictured,
                                        count,
                                        ..
                                    }) = encoded.runs[from..].last_mut()
                                        && *held == punch
                                        && pictured
                                            .as_ref()
                                            .map(|(image, smooth)| (image.id(), *smooth))
                                            == picture
                                                .as_ref()
                                                .map(|(image, smooth)| (image.id(), *smooth))
                                    {
                                        *count += 1;
                                        continue;
                                    }
                                    encoded.runs.push(ListRun::Quads {
                                        punch,
                                        picture,
                                        start: index,
                                        count: 1,
                                    });
                                }
                                Some(Encoding::Drawing {
                                    drawing,
                                    rect,
                                    clip,
                                }) => encoded.runs.push(ListRun::Drawing {
                                    drawing,
                                    rect,
                                    clip,
                                }),
                                None => {}
                            }
                        }
                    }
                    encoded.parts.push((from, encoded.runs.len()));
                }
            }
        }
    }

    fn release_drawings(&mut self) {
        let frames = self.frames;
        let lists = &mut self.lists;
        self.drawn.retain(|key| match lists.get(key) {
            Some(encoded) if encoded.seen == frames => true,
            Some(_) => {
                lists.remove(key);
                false
            }
            None => false,
        });
        self.frames += 1;
    }

    fn sweep(&mut self, output: &WindowOutput, pixels_per_point: f32) {
        let mut live = std::collections::HashSet::new();
        let at = Walk {
            factor: pixels_per_point,
            space: 0,
            origin: [0.0; 2],
            bound: OPEN,
            offset: Vec2::ZERO,
            clip: Rect::EVERYTHING,
            fade: Fade::NONE,
        };
        mark_layers(&output.layers, at, &mut live);
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
                uv: corner_radius,
                color: self.encode(color),
                params: [0.0, stroke_width, 0.0, 0.0],
                turn: turn(rotation),
            }),
            Quad::Glyph {
                rect,
                clip,
                color,
                glyph,
                turn: rotation,
            } => {
                let image = glyph.image()?;
                let (atlas, colored) = match image.color {
                    true => (&mut self.colors, 1.0),
                    false => (&mut self.atlas, 0.0),
                };
                let uv = atlas.insert(queue, glyph.id, image)?;
                plain(Instance {
                    rect,
                    clip,
                    uv,
                    color: self.encode(color),
                    params: [0.0, 0.0, 1.0, colored],
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
                    uv: [corner_radius; 4],
                    color: [0.0, 0.0, 0.0, 1.0],
                    params: [0.0, 0.0, 0.0, 0.0],
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
            Quad::Triangle {
                corners: [a, b, c],
                edges,
                clip,
                color,
            } => plain(Instance {
                rect: [a[0], a[1], b[0], b[1]],
                clip,
                uv: [c[0], c[1], 0.0, 0.0],
                color: self.encode(color),
                params: [f32::from(edges), 0.0, 4.0, 0.0],
                turn: turn(Turn::NONE),
            }),
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
            if let Some(draw) = drawing.draw::<Rc<dyn Draw>>() {
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
            self.draw_steps(pass, &self.steps, None);
            return;
        };
        for scissor in scissors {
            if clip(pass, *scissor) {
                self.draw_steps(pass, &self.steps, Some(*scissor));
            }
        }
    }

    fn draw_steps(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        steps: &[Step],
        scissor: Option<[u32; 4]>,
    ) {
        for step in steps {
            match step {
                Step::Runs(runs) => self.draw(pass, runs, scissor),
                Step::Filter { steps, .. } => self.draw_steps(pass, steps, scissor),
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
        if self.filters.is_empty() {
            self.effects.clear();
            let mut pass = self.begin(encoder, target, load);
            self.paint(&mut pass);
            return;
        }
        if self.empty() {
            return;
        }
        let scissors: Vec<Option<[u32; 4]>> = match &self.scissors {
            Some(scissors) => scissors.iter().copied().map(Some).collect(),
            None => vec![None],
        };
        let format = self.format;
        self.effects.truncate(self.filters.len());
        while self.effects.len() < self.filters.len() {
            self.effects.push(filter::Effects::new(device, format));
        }
        for effects in &mut self.effects {
            effects.ensure(device, size);
        }
        let steps = std::mem::take(&mut self.steps);
        let mut load = load;
        for scissor in scissors {
            self.render_steps(device, queue, encoder, target, load, scissor, &steps);
            load = wgpu::LoadOp::Load;
        }
        self.steps = steps;
    }

    #[allow(clippy::too_many_arguments)]
    fn render_steps(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        scissor: Option<[u32; 4]>,
        steps: &[Step],
    ) {
        let initial = load;
        let mut load = load;
        let mut loaded = false;
        for step in steps {
            match step {
                Step::Runs(runs) => {
                    let mut pass = self.begin(encoder, target, load);
                    if let Some(scissor) = scissor {
                        clip(&mut pass, scissor);
                    }
                    self.draw(&mut pass, runs, scissor);
                }
                Step::Filter {
                    index,
                    bottom,
                    steps: filtered,
                } => {
                    let Some(scene) = self.effects.get(*index).and_then(filter::Effects::scene)
                    else {
                        continue;
                    };
                    let scene = scene.clone();
                    let scene_load = match (self.whole, initial) {
                        (true, wgpu::LoadOp::Clear(color)) if *bottom => wgpu::LoadOp::Clear(color),
                        (true, _) => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        (false, _) => wgpu::LoadOp::Load,
                    };
                    self.render_steps(
                        device, queue, encoder, &scene, scene_load, scissor, filtered,
                    );
                    let (prepared, srgb) = (self.filters[*index], self.srgb);
                    if let Some(effects) = self.effects.get_mut(*index) {
                        effects.record(device, queue, encoder, &prepared, srgb, scissor);
                    }
                    let mut pass = self.begin(encoder, target, load);
                    if let Some(scissor) = scissor {
                        clip(&mut pass, scissor);
                    }
                    if let Some(effects) = self.effects.get(*index) {
                        effects.compose(&mut pass);
                    }
                }
            }
            load = wgpu::LoadOp::Load;
            loaded = true;
        }
        if !loaded {
            drop(self.begin(encoder, target, load));
        }
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
                if let (Some(draw), Some(at)) =
                    (drawing.draw::<Rc<dyn Draw>>(), within(*at, scissor))
                {
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

fn frame_run(punch: bool, picture: Option<(ImageId, bool)>, space: u32, at: usize) -> Run {
    Run {
        punch,
        picture,
        drawing: None,
        listed: false,
        space,
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
    mark_parts(&display.parts, at, live);
}

fn mark_parts(parts: &[Part], at: Walk, live: &mut std::collections::HashSet<ListKey>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(_, translation, child) => mark(child, at.child(*translation), live),
            Part::Group(group, inner) => mark_parts(inner, at.group(group, &mut |_, _, _| 0), live),
        }
    }
}

fn mark_layers(layers: &[Layer], at: Walk, live: &mut std::collections::HashSet<ListKey>) {
    for layer in layers {
        match layer {
            Layer::Shape(_) => {}
            Layer::Display(translation, display) => mark(display, at.child(*translation), live),
            Layer::Group(group, inner) => {
                mark_layers(inner, at.group(group, &mut |_, _, _| 0), live)
            }
        }
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

pub fn wgpu_color(color: Color32) -> wgpu::Color {
    wgpu_color_in(wgpu::TextureFormat::Rgba8UnormSrgb, color)
}

pub fn wgpu_color_in(format: wgpu::TextureFormat, color: Color32) -> wgpu::Color {
    let [red, green, blue, alpha] = match format.is_srgb() {
        true => color.to_linear_f32(),
        false => color.to_normalized_gamma_f32(),
    };
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
    colors: &wgpu::TextureView,
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
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(colors),
            },
        ],
    })
}

fn rgba(srgb: bool) -> wgpu::TextureFormat {
    match srgb {
        true => wgpu::TextureFormat::Rgba8UnormSrgb,
        false => wgpu::TextureFormat::Rgba8Unorm,
    }
}

#[cfg(test)]
mod tests;
