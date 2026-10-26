use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui_core::context::WindowOutput;
use beui_core::drawing::Drawing;
use beui_core::geometry::{Rect, vec2};
use beui_core::painting::{Painting, Paints};
use beui_core::window::WindowId;
use beui_renderer_wgpu::present::GpuSetup;
use beui_renderer_wgpu::{Draw, DrawAt, Repaint, Repainting, wgpu_color_in};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::floor;
use crate::head::EYE_HEIGHT;
use crate::space::SKY;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;

pub const SEGMENTS: u32 = 48;
const VERTICES: u32 = 2 * (SEGMENTS + 1);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChromeFrame {
    pub span: [f32; 4],
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct PanelFrame {
    pub window: WindowId,
    pub model: Mat4,
    pub metres: glam::Vec2,
    pub bend: f32,
    pub chrome: Option<ChromeFrame>,
}

#[derive(Clone)]
pub struct Frame {
    pub pixels_per_point: f32,
    pub windows: Vec<WindowOutput>,
    pub chrome: Vec<WindowOutput>,
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub cursor: Option<PanelFrame>,
    pub panels: Vec<PanelFrame>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_projection: [[f32; 4]; 4],
    eye: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    grid: [f32; 4],
    axis: [f32; 4],
    floor: [f32; 4],
    inverse: [[f32; 4]; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PanelUniform {
    model: [[f32; 4]; 4],
    shape: [f32; 4],
    span: [f32; 4],
    extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlitUniform {
    rect: [f32; 4],
    screen: [f32; 4],
}

struct Pending {
    output: WindowOutput,
    whole: bool,
}

struct Panel {
    renderer: beui_renderer_wgpu::Renderer,
    size: (u32, u32),
    texture: wgpu::Texture,
    level: wgpu::TextureView,
    mips: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    painted: bool,
}

struct Target {
    size: (u32, u32),
    color: wgpu::TextureView,
    resolved: wgpu::TextureView,
    depth: wgpu::TextureView,
    group: wgpu::BindGroup,
}

struct Pipelines {
    format: wgpu::TextureFormat,
    background: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    panel: wgpu::RenderPipeline,
    chrome: wgpu::RenderPipeline,
    cursor: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    linear: wgpu::Sampler,
    scene_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    mip_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    scene_uniform: wgpu::Buffer,
    scene_group: wgpu::BindGroup,
    blit_uniform: wgpu::Buffer,
    mip: wgpu::RenderPipeline,
    pipelines: Option<Pipelines>,
    target: Option<Target>,
    panels: HashMap<WindowId, Panel>,
    pending: HashMap<WindowId, Pending>,
    chrome: HashMap<WindowId, Panel>,
    chrome_pending: HashMap<WindowId, Pending>,
}

#[derive(Clone)]
pub struct Compositor(Rc<RefCell<Gpu>>);

impl Compositor {
    pub fn new(setup: &GpuSetup) -> Self {
        Self(Rc::new(RefCell::new(Gpu::new(setup))))
    }

    pub fn update(&self, output: &WindowOutput) -> bool {
        let mut gpu = self.0.borrow_mut();
        let known = gpu.panels.contains_key(&output.id) || gpu.pending.contains_key(&output.id);
        if known && !output.changed {
            return false;
        }
        let whole = !known || gpu.pending.contains_key(&output.id);
        gpu.pending.insert(
            output.id,
            Pending {
                output: output.clone(),
                whole,
            },
        );
        true
    }

    pub fn update_chrome(&self, output: &WindowOutput) {
        self.0.borrow_mut().chrome_pending.insert(
            output.id,
            Pending {
                output: output.clone(),
                whole: true,
            },
        );
    }

    pub fn forget(&self, window: WindowId) {
        let mut gpu = self.0.borrow_mut();
        gpu.panels.remove(&window);
        gpu.pending.remove(&window);
        gpu.chrome.remove(&window);
        gpu.chrome_pending.remove(&window);
    }

    pub fn drawing(&self, frame: Frame) -> Drawing {
        beui_renderer_wgpu::painted_drawing(SpaceDraw {
            gpu: self.clone(),
            frame,
        })
    }
}

#[cfg(test)]
impl Compositor {
    pub(crate) fn paint_windows(&self) {
        let mut gpu = self.0.borrow_mut();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        gpu.paint_windows(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    pub(crate) fn read(&self, window: WindowId) -> Option<beui_core::app::automation::Capture> {
        let gpu = self.0.borrow();
        let panel = gpu.panels.get(&window)?;
        beui_renderer_wgpu::present::read_texture(&gpu.device, &gpu.queue, &panel.texture)
    }

    pub(crate) fn drawn(&self, window: WindowId) -> Option<(u64, (u32, u32))> {
        let gpu = self.0.borrow();
        let panel = gpu.panels.get(&window)?;
        Some((panel.renderer.id(), panel.size))
    }
}

struct SpaceDraw {
    gpu: Compositor,
    frame: Frame,
}

impl Paints for SpaceDraw {
    fn painting(&self, rect: Rect) -> Option<Painting> {
        crate::picture::painting(&self.frame, rect)
    }
}

impl Draw for SpaceDraw {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let mut gpu = self.gpu.0.borrow_mut();
        gpu.paint_windows(encoder);
        gpu.render(encoder, &self.frame, at);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt) {
        let gpu = self.gpu.0.borrow();
        let (Some(pipelines), Some(target)) = (&gpu.pipelines, &gpu.target) else {
            return;
        };
        if pipelines.format != at.format {
            return;
        }
        pass.set_pipeline(&pipelines.blit);
        pass.set_bind_group(0, &target.group, &[]);
        pass.draw(0..4, 0..1);
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniform_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[derive(Clone, Copy)]
struct Stage<'a> {
    label: &'a str,
    vertex: &'a str,
    fragment: &'a str,
    layouts: &'a [&'a wgpu::BindGroupLayout],
    format: wgpu::TextureFormat,
    samples: u32,
    depth: Option<bool>,
    blend: Option<wgpu::BlendState>,
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    stage: Stage,
) -> wgpu::RenderPipeline {
    pipeline_comparing(device, shader, stage, wgpu::CompareFunction::LessEqual)
}

fn pipeline_comparing(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    stage: Stage,
    compare: wgpu::CompareFunction,
) -> wgpu::RenderPipeline {
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> =
        stage.layouts.iter().copied().map(Some).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(stage.label),
        bind_group_layouts: &layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(stage.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(stage.vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: stage.depth.map(|write| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: stage.samples,
            ..wgpu::MultisampleState::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(stage.fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: stage.format,
                blend: stage.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Gpu {
    fn new(setup: &GpuSetup) -> Self {
        let device = setup.device.clone();
        let shader = device.create_shader_module(wgpu::include_wgsl!("space.wgsl"));
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("space scene"),
            entries: &[uniform_entry(0)],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("space panel"),
            entries: &[uniform_entry(0), texture_entry(1), sampler_entry(2)],
        });
        let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("space mip"),
            entries: &[texture_entry(1), sampler_entry(2)],
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("space blit"),
            entries: &[uniform_entry(0), texture_entry(1), sampler_entry(2)],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("space panel"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..wgpu::SamplerDescriptor::default()
        });
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("space linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let scene_uniform = uniform_buffer(&device, "space scene", size_of::<SceneUniform>());
        let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("space scene"),
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_uniform.as_entire_binding(),
            }],
        });
        let blit_uniform = uniform_buffer(&device, "space blit", size_of::<BlitUniform>());
        let mip = pipeline(
            &device,
            &shader,
            Stage {
                label: "space mip",
                vertex: "mip_vertex",
                fragment: "mip_fragment",
                layouts: &[&mip_layout],
                format: setup.format,
                samples: 1,
                depth: None,
                blend: None,
            },
        );
        Self {
            device,
            queue: setup.queue.clone(),
            format: setup.format,
            shader,
            sampler,
            linear,
            scene_layout,
            texture_layout,
            mip_layout,
            blit_layout,
            scene_uniform,
            scene_group,
            blit_uniform,
            mip,
            pipelines: None,
            target: None,
            panels: HashMap::new(),
            pending: HashMap::new(),
            chrome: HashMap::new(),
            chrome_pending: HashMap::new(),
        }
    }

    fn pipelines(&mut self, format: wgpu::TextureFormat) {
        if self
            .pipelines
            .as_ref()
            .is_some_and(|pipelines| pipelines.format == format)
        {
            return;
        }
        let device = &self.device;
        let shader = &self.shader;
        let background = Stage {
            label: "space background",
            vertex: "background_vertex",
            fragment: "background_fragment",
            layouts: &[&self.scene_layout],
            format,
            samples: SAMPLES,
            depth: Some(false),
            blend: None,
        };
        let grid = Stage {
            label: "space grid",
            vertex: "grid_vertex",
            fragment: "grid_fragment",
            layouts: &[&self.scene_layout],
            format,
            samples: SAMPLES,
            depth: Some(false),
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        };
        let panel = Stage {
            label: "space panel",
            vertex: "panel_vertex",
            fragment: "panel_fragment",
            layouts: &[&self.scene_layout, &self.texture_layout],
            format,
            samples: SAMPLES,
            depth: Some(true),
            blend: None,
        };
        self.target = None;
        self.pipelines = Some(Pipelines {
            format,
            background: pipeline_comparing(
                device,
                shader,
                background,
                wgpu::CompareFunction::Always,
            ),
            grid: pipeline_comparing(device, shader, grid, wgpu::CompareFunction::Always),
            panel: pipeline(device, shader, panel),
            chrome: pipeline(
                device,
                shader,
                Stage {
                    label: "space chrome",
                    vertex: "panel_vertex",
                    fragment: "chrome_fragment",
                    layouts: &[&self.scene_layout, &self.texture_layout],
                    format,
                    samples: SAMPLES,
                    depth: Some(false),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                },
            ),
            cursor: pipeline_comparing(
                device,
                shader,
                Stage {
                    label: "space cursor",
                    vertex: "panel_vertex",
                    fragment: "cursor_fragment",
                    layouts: &[&self.scene_layout, &self.texture_layout],
                    format,
                    samples: SAMPLES,
                    depth: Some(false),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                },
                wgpu::CompareFunction::Always,
            ),
            blit: pipeline(
                device,
                shader,
                Stage {
                    label: "space blit",
                    vertex: "blit_vertex",
                    fragment: "blit_fragment",
                    layouts: &[&self.blit_layout],
                    format,
                    samples: 1,
                    depth: None,
                    blend: None,
                },
            ),
        });
    }

    fn target(&mut self, size: (u32, u32), format: wgpu::TextureFormat) {
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let texture = |label, format, samples, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let multisampled = texture(
            "space samples",
            format,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let resolved = texture(
            "space scene",
            format,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture(
            "space depth",
            DEPTH,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("space blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.blit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&resolved),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.linear),
                },
            ],
        });
        self.target = Some(Target {
            size,
            color: multisampled,
            resolved,
            depth,
            group,
        });
    }

    fn sheets(&mut self, chrome: bool) -> &mut HashMap<WindowId, Panel> {
        match chrome {
            true => &mut self.chrome,
            false => &mut self.panels,
        }
    }

    fn panel(&mut self, chrome: bool, window: WindowId, size: (u32, u32)) {
        if self
            .sheets(chrome)
            .get(&window)
            .is_some_and(|panel| panel.size == size)
        {
            return;
        }
        let levels = 32 - size.0.max(size.1).leading_zeros();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("space window"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let level = |base: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: base,
                mip_level_count: Some(1),
                ..wgpu::TextureViewDescriptor::default()
            })
        };
        let mips = (1..levels)
            .map(|index| {
                let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("space mip"),
                    layout: &self.mip_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&level(index - 1)),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.linear),
                        },
                    ],
                });
                (level(index), group)
            })
            .collect();
        let (renderer, uniform) = match self.sheets(chrome).remove(&window) {
            Some(panel) => (panel.renderer, panel.uniform),
            None => (
                beui_renderer_wgpu::Renderer::new(&self.device, self.format),
                uniform_buffer(&self.device, "space panel", size_of::<PanelUniform>()),
            ),
        };
        let whole = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("space panel"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&whole),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let level = level(0);
        self.sheets(chrome).insert(
            window,
            Panel {
                renderer,
                size,
                texture,
                level,
                mips,
                uniform,
                group,
                painted: false,
            },
        );
    }

    fn paint_windows(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let largest = self.device.limits().max_texture_dimension_2d;
        let pending: Vec<(bool, WindowId, Pending)> = self
            .pending
            .drain()
            .map(|(window, pending)| (false, window, pending))
            .chain(
                self.chrome_pending
                    .drain()
                    .map(|(window, pending)| (true, window, pending)),
            )
            .collect();
        for (chrome, window, pending) in pending {
            let output = &pending.output;
            let scale = output.pixels_per_point;
            let size = (
                ((output.size.x * scale).round() as u32).clamp(1, largest),
                ((output.size.y * scale).round() as u32).clamp(1, largest),
            );
            self.panel(chrome, window, size);
            let sheets = match chrome {
                true => &mut self.chrome,
                false => &mut self.panels,
            };
            let Some(panel) = sheets.get_mut(&window) else {
                continue;
            };
            let background = output.window.appearance.clear_color;
            let (repaint, moved) = match pending.whole || !panel.painted {
                true => (Repaint::Everything, None),
                false => output.repaint_moving(background),
            };
            let repaint = panel.renderer.prepare(
                &self.device,
                &self.queue,
                output,
                vec2(size.0 as f32, size.1 as f32),
                scale,
                repaint,
            );
            let load = match repaint {
                Repaint::Everything => wgpu::LoadOp::Clear(wgpu_color_in(self.format, background)),
                Repaint::Region { .. } => {
                    if let Some(moved) = moved {
                        panel
                            .renderer
                            .shift(&self.device, encoder, &panel.texture, moved, scale);
                    }
                    wgpu::LoadOp::Load
                }
            };
            panel
                .renderer
                .render(&self.device, &self.queue, encoder, &panel.level, size, load);
            panel.painted = true;
            for (view, group) in &panel.mips {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("space mip"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.mip);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
    }

    fn render(&mut self, encoder: &mut wgpu::CommandEncoder, frame: &Frame, at: DrawAt) {
        let size = (at.width().max(1), at.height().max(1));
        self.pipelines(at.format);
        self.target(size, at.format);
        let (Some(pipelines), Some(target)) = (&self.pipelines, &self.target) else {
            return;
        };
        let scene = SceneUniform {
            view_projection: frame.view_projection.to_cols_array_2d(),
            eye: frame.eye.extend(1.0).to_array(),
            right: frame.right.extend(0.0).to_array(),
            up: frame.up.extend(0.0).to_array(),
            sky: SKY.to_linear_f32(),
            ground: floor::GROUND.to_linear_f32(),
            grid: floor::GRID.to_linear_f32(),
            axis: floor::AXIS.to_linear_f32(),
            floor: [
                floor::EXTENT,
                floor::FADE_START,
                floor::FADE_END,
                EYE_HEIGHT,
            ],
            inverse: frame.view_projection.inverse().to_cols_array_2d(),
            options: [f32::from(u8::from(at.format.is_srgb())), 0.0, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.scene_uniform, 0, bytemuck::bytes_of(&scene));
        self.queue.write_buffer(
            &self.blit_uniform,
            0,
            bytemuck::bytes_of(&BlitUniform {
                rect: at.rect,
                screen: [at.screen.x, at.screen.y, 0.0, 0.0],
            }),
        );
        for panel in frame.panels.iter().chain(&frame.cursor) {
            let uniform = |lift: f32, span: [f32; 4], opacity: f32| PanelUniform {
                model: panel.model.to_cols_array_2d(),
                shape: [panel.metres.x, panel.metres.y, panel.bend, lift],
                span,
                extra: [opacity, 0.0, 0.0, 0.0],
            };
            if let Some(drawn) = self.panels.get(&panel.window) {
                self.queue.write_buffer(
                    &drawn.uniform,
                    0,
                    bytemuck::bytes_of(&uniform(0.0, [0.0, 0.0, 1.0, 1.0], 1.0)),
                );
            }
            if let (Some(chrome), Some(drawn)) = (panel.chrome, self.chrome.get(&panel.window)) {
                self.queue.write_buffer(
                    &drawn.uniform,
                    0,
                    bytemuck::bytes_of(&uniform(crate::chrome::LIFT, chrome.span, chrome.opacity)),
                );
            }
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("space"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color,
                depth_slice: None,
                resolve_target: Some(&target.resolved),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu_color_in(at.format, SKY)),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.scene_group, &[]);
        pass.set_pipeline(&pipelines.background);
        pass.draw(0..4, 0..1);
        pass.set_pipeline(&pipelines.grid);
        pass.draw(0..4, 0..1);
        let sheets = [
            (&pipelines.panel, &self.panels),
            (&pipelines.chrome, &self.chrome),
        ];
        for (pipeline, drawn) in sheets {
            pass.set_pipeline(pipeline);
            for panel in &frame.panels {
                if let Some(drawn) = drawn.get(&panel.window)
                    && drawn.painted
                {
                    pass.set_bind_group(1, &drawn.group, &[]);
                    pass.draw(0..VERTICES, 0..1);
                }
            }
        }
        if let Some(cursor) = &frame.cursor
            && let Some(drawn) = self.panels.get(&cursor.window)
            && drawn.painted
        {
            pass.set_pipeline(&pipelines.cursor);
            pass.set_bind_group(1, &drawn.group, &[]);
            pass.draw(0..VERTICES, 0..1);
        }
    }
}
