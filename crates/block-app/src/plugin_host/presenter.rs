use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use beui::{DrawAt, Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{ScreenId, ScreenLayout};

use super::backend::{Availability, Frame};

#[cfg(not(target_arch = "wasm32"))]
use super::wasm::{Presenter as PlatformPresenter, presenter as build_presenter};
#[cfg(target_arch = "wasm32")]
use super::web::renderer::{
    WebSurfacePresenter as PlatformPresenter, presenter as build_presenter,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PresenterState {
    Waiting,
    Presenting,
    Unsupported(String),
    Failed(String),
    Released,
}

#[derive(Clone)]
pub(super) struct PresenterStatus(Arc<Mutex<PresenterState>>);

impl PresenterStatus {
    pub(super) fn waiting() -> Self {
        Self(Arc::new(Mutex::new(PresenterState::Waiting)))
    }

    pub(super) fn get(&self) -> PresenterState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: PresenterState) {
        *self.0.lock().unwrap() = state;
    }
}

pub(super) trait SurfacePresenter {
    type Frame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        pipeline: &BlitPipeline,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn texture(&self, surface: u32) -> Option<&wgpu::BindGroup>;

    fn release(&mut self, surface: u32);
}

pub(super) const MAX_SURFACES: u32 = 8;
const MAX_PENDING_FRAMES: usize = 8;
const REGION_BYTES: u64 = 64;

pub(super) struct BlitPipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) texture_layout: wgpu::BindGroupLayout,
    pub(super) regions_layout: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
    stride: u32,
    #[cfg(target_arch = "wasm32")]
    target_format: wgpu::TextureFormat,
}

impl BlitPipeline {
    pub(super) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hosted plugin surface layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let regions_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hosted plugin region layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(REGION_BYTES),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hosted plugin surface pipeline layout"),
            bind_group_layouts: &[Some(&texture_layout), Some(&regions_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hosted plugin surface pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blit_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("blit_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let stride = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(REGION_BYTES as u32);
        Self {
            pipeline,
            texture_layout,
            regions_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hosted plugin surface sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            stride,
            #[cfg(target_arch = "wasm32")]
            target_format,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn copy_format(&self) -> wgpu::TextureFormat {
        match self.target_format.is_srgb() {
            true => wgpu::TextureFormat::Rgba8UnormSrgb,
            false => wgpu::TextureFormat::Rgba8Unorm,
        }
    }

    pub(super) fn texture_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hosted plugin surface bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Quad {
    pub(crate) rect: Rect,
    pub(crate) corners: [Pos2; 4],
    pub(crate) opacity: f32,
}

impl Quad {
    pub(crate) fn upright(rect: Rect) -> Self {
        Self {
            rect,
            corners: [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            opacity: 1.0,
        }
    }

    pub(crate) fn crop_to(self, clip: Rect) -> Option<(Self, Rect)> {
        let horizontal = self.corners[1] - self.corners[0];
        let vertical = self.corners[3] - self.corners[0];
        let determinant = horizontal.x * vertical.y - horizontal.y * vertical.x;
        if determinant.abs() <= f32::EPSILON {
            return None;
        }
        let to_uv = |point: Pos2| {
            let delta = point - self.corners[0];
            pos2(
                (delta.x * vertical.y - delta.y * vertical.x) / determinant,
                (horizontal.x * delta.y - horizontal.y * delta.x) / determinant,
            )
        };
        let source = Rect::from_points(&[
            to_uv(clip.left_top()),
            to_uv(clip.right_top()),
            to_uv(clip.right_bottom()),
            to_uv(clip.left_bottom()),
        ])
        .intersect(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)));
        if source.width() <= 0.0 || source.height() <= 0.0 {
            return None;
        }
        let point = |u: f32, v: f32| self.corners[0] + horizontal * u + vertical * v;
        let corners = [
            point(source.min.x, source.min.y),
            point(source.max.x, source.min.y),
            point(source.max.x, source.max.y),
            point(source.min.x, source.max.y),
        ];
        Some((
            Self {
                rect: Rect::from_points(&corners),
                corners,
                opacity: self.opacity,
            },
            source,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Region {
    pub(super) offset: [f32; 2],
    pub(super) scale: [f32; 2],
    pub(super) quad: Quad,
}

impl Region {
    pub(super) fn of(
        layout: &ScreenLayout,
        screen: ScreenId,
        quad: Quad,
        source: Rect,
    ) -> Option<Self> {
        if layout.is_empty() {
            return None;
        }
        let placement = layout.placement(screen)?;
        let width = layout.width as f32;
        let height = layout.height as f32;
        let left = placement.x as f32 + placement.width as f32 * source.min.x;
        let top = placement.y as f32 + placement.height as f32 * source.min.y;
        Some(Self {
            offset: [left / width, top / height],
            scale: [
                placement.width as f32 * source.width() / width,
                placement.height as f32 * source.height() / height,
            ],
            quad,
        })
    }

    fn values(&self, at: &DrawAt) -> [f32; 16] {
        let scale = at.pixels_per_point;
        let screen: Vec2 = vec2(at.screen.x.max(1.0), at.screen.y.max(1.0));
        let mut values = [0.0; 16];
        values[..2].copy_from_slice(&self.offset);
        values[2..4].copy_from_slice(&self.scale);
        for (index, corner) in self.quad.corners.iter().enumerate() {
            values[4 + index * 2] = corner.x * scale / screen.x * 2.0 - 1.0;
            values[5 + index * 2] = 1.0 - corner.y * scale / screen.y * 2.0;
        }
        values[12] = self.quad.opacity.clamp(0.0, 1.0);
        values
    }
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) layout: ScreenLayout,
    pub(super) frames: Vec<Frame>,
}

impl Shared {
    pub(super) fn publish(&mut self, layout: &ScreenLayout, frame: Option<Frame>) {
        self.layout.clone_from(layout);
        let Some(frame) = frame else {
            return;
        };
        if self.frames.len() >= MAX_PENDING_FRAMES {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }
}

#[derive(Clone)]
pub(crate) struct Blit {
    pub(super) surface: u32,
    pub(super) status: PresenterStatus,
    pub(super) shared: Arc<Mutex<Shared>>,
    pub(super) screen: ScreenId,
    pub(super) quad: Quad,
    pub(super) source: Rect,
    pub(super) drawn: Option<(u32, u32)>,
}

impl PartialEq for Blit {
    fn eq(&self, other: &Self) -> bool {
        self.surface == other.surface
            && Arc::ptr_eq(&self.shared, &other.shared)
            && self.screen == other.screen
            && self.quad == other.quad
            && self.source == other.source
            && self.drawn == other.drawn
    }
}

impl Blit {
    pub(crate) fn pending(&self) -> bool {
        !self.shared.lock().unwrap().frames.is_empty()
    }
}

struct Regions {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    capacity: usize,
}

pub(crate) struct PluginDrawing {
    blits: Vec<Blit>,
    regions: RefCell<Option<Regions>>,
    placed: RefCell<Vec<Option<u32>>>,
}

impl PluginDrawing {
    pub(crate) fn new(blits: Vec<Blit>) -> Self {
        Self {
            blits,
            regions: RefCell::new(None),
            placed: RefCell::new(Vec::new()),
        }
    }
}

impl beui::Draw for PluginDrawing {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        PRESENTER.with(|presenter| {
            let mut presenter = presenter.borrow_mut();
            let Some(presenter) = presenter.as_mut() else {
                for blit in &self.blits {
                    blit.status.set(PresenterState::Unsupported(
                        "The active renderer has no plugin surface presenter.".to_owned(),
                    ));
                }
                return;
            };
            let mut regions = self.regions.borrow_mut();
            let needed = self.blits.len().max(1);
            if regions
                .as_ref()
                .is_none_or(|regions| regions.capacity < needed)
            {
                let capacity = needed.next_power_of_two();
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("plugin surface regions"),
                    size: u64::from(presenter.pipeline.stride) * capacity as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("plugin surface regions"),
                    layout: &presenter.pipeline.regions_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(REGION_BYTES),
                        }),
                    }],
                });
                *regions = Some(Regions {
                    buffer,
                    group,
                    capacity,
                });
            }
            let regions = regions.as_ref().expect("the regions were just created");
            let mut placed = self.placed.borrow_mut();
            placed.clear();
            for (index, blit) in self.blits.iter().enumerate() {
                let (frames, region) = {
                    let mut shared = blit.shared.lock().unwrap();
                    let frames = std::mem::take(&mut shared.frames);
                    let region = shared.layout.placement(blit.screen).and_then(|placement| {
                        Region::of(&shared.layout, blit.screen, blit.quad, blit.source).filter(
                            |_| {
                                blit.drawn.is_none_or(|drawn| {
                                    drawn == (placement.width, placement.height)
                                })
                            },
                        )
                    });
                    (frames, region)
                };
                let mut failure = None;
                for frame in &frames {
                    let applied = presenter
                        .replace(device, blit.surface, frame)
                        .and_then(|()| presenter.prepare(queue, blit.surface, frame));
                    if let Err(error) = applied {
                        failure = Some(error);
                    }
                }
                match failure {
                    Some(error) => blit.status.set(PresenterState::Failed(error)),
                    None if presenter.platform.is_some() => {
                        blit.status.set(PresenterState::Presenting);
                    }
                    None => blit.status.set(PresenterState::Unsupported(UNSUPPORTED.to_owned())),
                }
                placed.push(region.map(|region| {
                    let offset = presenter.pipeline.stride * index as u32;
                    queue.write_buffer(
                        &regions.buffer,
                        u64::from(offset),
                        bytemuck::cast_slice(&region.values(&at)),
                    );
                    offset
                }));
            }
        });
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        PRESENTER.with(|presenter| {
            let presenter = presenter.borrow();
            let Some(presenter) = presenter.as_ref() else {
                return;
            };
            let Some(platform) = &presenter.platform else {
                return;
            };
            let regions = self.regions.borrow();
            let Some(regions) = regions.as_ref() else {
                return;
            };
            let placed = self.placed.borrow();
            for (blit, offset) in self.blits.iter().zip(placed.iter()) {
                let (Some(offset), Some(texture)) = (offset, platform.texture(blit.surface)) else {
                    continue;
                };
                pass.set_pipeline(&presenter.pipeline.pipeline);
                pass.set_bind_group(0, texture, &[]);
                pass.set_bind_group(1, &regions.group, &[*offset]);
                pass.draw(0..6, 0..1);
            }
        });
    }
}

thread_local! {
    static PRESENTER: RefCell<Option<Presenter>> = const { RefCell::new(None) };
}

struct Presenter {
    pipeline: BlitPipeline,
    platform: Option<PlatformPresenter>,
}

pub(super) fn install(setup: &beui::Setup) -> Availability {
    let pipeline = BlitPipeline::new(&setup.device, setup.format);
    let platform = build_presenter(&setup.device, &setup.queue);
    let availability = Availability(platform.as_ref().map(|_| ()).map_err(Clone::clone));
    PRESENTER.with(|presenter| {
        *presenter.borrow_mut() = Some(Presenter {
            pipeline,
            platform: platform.ok(),
        });
    });
    availability
}

pub(super) fn release(surface: u32, status: &PresenterStatus) {
    PRESENTER.with(|presenter| {
        if let Some(platform) = presenter
            .borrow_mut()
            .as_mut()
            .and_then(|presenter| presenter.platform.as_mut())
        {
            platform.release(surface);
        }
    });
    status.set(PresenterState::Released);
}

impl Presenter {
    fn replace(
        &mut self,
        device: &wgpu::Device,
        surface: u32,
        frame: &Frame,
    ) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.replace(device, &self.pipeline, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }

    fn prepare(&mut self, queue: &wgpu::Queue, surface: u32, frame: &Frame) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.prepare(queue, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }
}

const UNSUPPORTED: &str = "This build has no presenter for that plugin surface.";
