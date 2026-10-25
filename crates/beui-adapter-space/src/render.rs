use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui_core::context::WindowOutput;
use beui_core::drawing::Drawing;
use beui_core::geometry::vec2;
use beui_core::window::WindowId;
use beui_renderer_wgpu::present::GpuSetup;
use beui_renderer_wgpu::{Draw, DrawAt, Repaint, Repainting, wgpu_color_in};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::space::SKY;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;

#[derive(Clone, Debug)]
pub struct PanelFrame {
    pub window: WindowId,
    pub model: Mat4,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub cursor: Option<(Vec3, f32)>,
    pub panels: Vec<PanelFrame>,
    pub lifted: Option<WindowId>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_projection: [[f32; 4]; 4],
    eye: [f32; 4],
    cursor: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    sky: [f32; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PanelUniform {
    model: [[f32; 4]; 4],
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
    floor: wgpu::RenderPipeline,
    panel: wgpu::RenderPipeline,
    lifted: wgpu::RenderPipeline,
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

    pub fn forget(&self, window: WindowId) {
        let mut gpu = self.0.borrow_mut();
        gpu.panels.remove(&window);
        gpu.pending.remove(&window);
    }

    pub fn drawing(&self, frame: Frame) -> Drawing {
        beui_renderer_wgpu::drawing(SpaceDraw {
            gpu: self.clone(),
            frame,
        })
    }
}

struct SpaceDraw {
    gpu: Compositor,
    frame: Frame,
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
        let scene = Stage {
            label: "space floor",
            vertex: "floor_vertex",
            fragment: "floor_fragment",
            layouts: &[&self.scene_layout],
            format,
            samples: SAMPLES,
            depth: Some(true),
            blend: None,
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
            floor: pipeline(device, shader, scene),
            panel: pipeline(device, shader, panel),
            lifted: pipeline_comparing(
                device,
                shader,
                Stage {
                    label: "space lifted panel",
                    ..panel
                },
                wgpu::CompareFunction::Always,
            ),
            cursor: pipeline(
                device,
                shader,
                Stage {
                    label: "space cursor",
                    vertex: "cursor_vertex",
                    fragment: "cursor_fragment",
                    layouts: &[&self.scene_layout],
                    format,
                    samples: SAMPLES,
                    depth: Some(false),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                },
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

    fn panel(&mut self, window: WindowId, size: (u32, u32)) {
        if self
            .panels
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
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
        let uniform = match self.panels.remove(&window) {
            Some(panel) => panel.uniform,
            None => uniform_buffer(&self.device, "space panel", size_of::<PanelUniform>()),
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
        self.panels.insert(
            window,
            Panel {
                renderer: beui_renderer_wgpu::Renderer::new(&self.device, self.format),
                size,
                level: level(0),
                mips,
                uniform,
                group,
                painted: false,
            },
        );
    }

    fn paint_windows(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let largest = self.device.limits().max_texture_dimension_2d;
        let pending: Vec<(WindowId, Pending)> = self.pending.drain().collect();
        for (window, pending) in pending {
            let output = &pending.output;
            let scale = output.pixels_per_point;
            let size = (
                ((output.size.x * scale).round() as u32).clamp(1, largest),
                ((output.size.y * scale).round() as u32).clamp(1, largest),
            );
            self.panel(window, size);
            let Some(panel) = self.panels.get_mut(&window) else {
                continue;
            };
            let background = output.window.appearance.clear_color;
            let repaint = match pending.whole || !panel.painted {
                true => Repaint::Everything,
                false => output.repaint(background),
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
                Repaint::Region { .. } => wgpu::LoadOp::Load,
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
        let (cursor, radius) = frame.cursor.unwrap_or((Vec3::ZERO, 0.0));
        let scene = SceneUniform {
            view_projection: frame.view_projection.to_cols_array_2d(),
            eye: frame.eye.extend(1.0).to_array(),
            cursor: cursor.extend(radius).to_array(),
            right: frame.right.extend(0.0).to_array(),
            up: frame.up.extend(0.0).to_array(),
            sky: SKY.to_linear_f32(),
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
        for panel in &frame.panels {
            if let Some(drawn) = self.panels.get(&panel.window) {
                self.queue.write_buffer(
                    &drawn.uniform,
                    0,
                    bytemuck::bytes_of(&PanelUniform {
                        model: panel.model.to_cols_array_2d(),
                    }),
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
        pass.set_pipeline(&pipelines.floor);
        pass.draw(0..4, 0..1);
        let lifted = |panel: &&PanelFrame| Some(panel.window) == frame.lifted;
        let (above, below): (Vec<&PanelFrame>, _) = frame.panels.iter().partition(lifted);
        for (pipeline, panels) in [(&pipelines.panel, below), (&pipelines.lifted, above)] {
            pass.set_pipeline(pipeline);
            for panel in panels {
                if let Some(drawn) = self.panels.get(&panel.window)
                    && drawn.painted
                {
                    pass.set_bind_group(1, &drawn.group, &[]);
                    pass.draw(0..4, 0..1);
                }
            }
        }
        if frame.cursor.is_some() {
            pass.set_pipeline(&pipelines.cursor);
            pass.draw(0..4, 0..1);
        }
    }
}
