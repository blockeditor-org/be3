use std::cell::RefCell;
use std::rc::Rc;

use crate::{Draw, DrawAt};

const PLACED_BYTES: u64 = 80;
const KEPT_RENDERERS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub clip: [f32; 4],
    pub rotation: f32,
    pub opacity: f32,
    pub corner_radius: f32,
    pub opaque: bool,
    pub srgb_encoded: bool,
}

impl Default for Quad {
    fn default() -> Self {
        Self {
            rect: [0.0; 4],
            uv: [0.0, 0.0, 1.0, 1.0],
            clip: [f32::MIN, f32::MIN, f32::MAX, f32::MAX],
            rotation: 0.0,
            opacity: 1.0,
            corner_radius: 0.0,
            opaque: false,
            srgb_encoded: false,
        }
    }
}

impl Quad {
    fn turn(&self) -> [f32; 2] {
        let snap = |value: f32| match (value.round() - value).abs() < 1e-6 {
            true => value.round(),
            false => value,
        };
        let (sin, cos) = self.rotation.sin_cos();
        [snap(cos), snap(sin)]
    }

    pub fn visible(&self) -> Option<[f32; 4]> {
        let [left, top, right, bottom] = self.rect;
        if right <= left || bottom <= top {
            return None;
        }
        let [cos, sin] = self.turn();
        let (half_x, half_y) = ((right - left) * 0.5, (bottom - top) * 0.5);
        let reach_x = half_x * cos.abs() + half_y * sin.abs();
        let reach_y = half_x * sin.abs() + half_y * cos.abs();
        let (center_x, center_y) = ((left + right) * 0.5, (top + bottom) * 0.5);
        let shown = [
            (center_x - reach_x).max(self.clip[0]),
            (center_y - reach_y).max(self.clip[1]),
            (center_x + reach_x).min(self.clip[2]),
            (center_y + reach_y).min(self.clip[3]),
        ];
        (shown[2] > shown[0] && shown[3] > shown[1]).then_some(shown)
    }

    fn bytes(&self, format: wgpu::TextureFormat, screen: [f32; 2]) -> Vec<u8> {
        let flag = |on: bool| f32::from(u8::from(on));
        let [cos, sin] = self.turn();
        let values: [f32; 20] = [
            self.rect[0],
            self.rect[1],
            self.rect[2],
            self.rect[3],
            self.uv[0],
            self.uv[1],
            self.uv[2],
            self.uv[3],
            self.clip[0],
            self.clip[1],
            self.clip[2],
            self.clip[3],
            screen[0].max(1.0),
            screen[1].max(1.0),
            cos,
            sin,
            self.opacity.clamp(0.0, 1.0),
            flag(self.opaque),
            flag(self.srgb_encoded && format.is_srgb()),
            self.corner_radius.max(0.0),
        ];
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Bound(wgpu::BindGroup);

pub struct Pipelines {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    texture_layout: wgpu::BindGroupLayout,
    placed_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    stride: u64,
    blend: wgpu::BlendState,
    formats: RefCell<Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>>,
}

thread_local! {
    static RENDERERS: RefCell<Vec<(u64, Rc<Pipelines>)>> = const { RefCell::new(Vec::new()) };
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Self {
        Self::blending(device, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
    }

    pub fn erasing(device: &wgpu::Device) -> Self {
        let erase = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        Self::blending(
            device,
            wgpu::BlendState {
                color: erase,
                alpha: erase,
            },
        )
    }

    fn blending(device: &wgpu::Device, blend: wgpu::BlendState) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("texture_quad.wgsl"));
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture quad"),
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
        let placed_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture quad placement"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(PLACED_BYTES),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("texture quad"),
            bind_group_layouts: &[Some(&texture_layout), Some(&placed_layout)],
            immediate_size: 0,
        });
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        Self {
            device: device.clone(),
            shader,
            texture_layout,
            placed_layout,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("texture quad"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            stride: PLACED_BYTES.div_ceil(alignment) * alignment,
            blend,
            formats: RefCell::new(Vec::new()),
        }
    }

    pub fn of(device: &wgpu::Device, at: &DrawAt) -> Rc<Self> {
        RENDERERS.with_borrow_mut(|renderers| {
            if let Some((_, pipelines)) = renderers.iter().find(|(id, _)| *id == at.renderer) {
                return pipelines.clone();
            }
            let pipelines = Rc::new(Self::new(device));
            if renderers.len() == KEPT_RENDERERS {
                renderers.remove(0);
            }
            renderers.push((at.renderer, pipelines.clone()));
            pipelines
        })
    }

    pub fn bind(&self, texture: &wgpu::Texture) -> Bound {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Bound(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture quad"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }))
    }

    fn pipeline(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let mut formats = self.formats.borrow_mut();
        if let Some((_, pipeline)) = formats.iter().find(|(known, _)| *known == format) {
            return pipeline.clone();
        }
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("texture quad"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(self.blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        formats.push((format, pipeline.clone()));
        pipeline
    }

    pub fn paint(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        texture: &Bound,
        quad: Quad,
    ) {
        let quads = Quads::default();
        quads.prepare(
            self,
            queue,
            format,
            [size.0 as f32, size.1 as f32],
            [(texture, quad)],
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("texture quad"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        quads.paint(&mut pass);
    }
}

struct Prepared {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    capacity: usize,
    pipeline: Option<wgpu::RenderPipeline>,
    drawn: Vec<(wgpu::BindGroup, u32)>,
}

#[derive(Default)]
pub struct Quads(RefCell<Option<Prepared>>);

impl Quads {
    pub fn prepare<'a>(
        &self,
        pipelines: &Pipelines,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        screen: [f32; 2],
        quads: impl IntoIterator<Item = (&'a Bound, Quad)>,
    ) {
        let shown: Vec<_> = quads
            .into_iter()
            .filter(|(_, quad)| quad.visible().is_some())
            .collect();
        let mut held = self.0.borrow_mut();
        let needed = shown.len().max(1);
        if held.as_ref().is_none_or(|held| held.capacity < needed) {
            let capacity = needed.next_power_of_two();
            let buffer = pipelines.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("texture quad placements"),
                size: pipelines.stride * capacity as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = pipelines
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("texture quad placements"),
                    layout: &pipelines.placed_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(PLACED_BYTES),
                        }),
                    }],
                });
            *held = Some(Prepared {
                buffer,
                group,
                capacity,
                pipeline: None,
                drawn: Vec::new(),
            });
        }
        let held = held.as_mut().expect("the placements were just made");
        let mut bytes = vec![0; (pipelines.stride as usize) * shown.len()];
        held.drawn.clear();
        for (index, (bound, quad)) in shown.iter().enumerate() {
            let offset = index * pipelines.stride as usize;
            let placed = quad.bytes(format, screen);
            bytes[offset..offset + placed.len()].copy_from_slice(&placed);
            held.drawn.push((bound.0.clone(), offset as u32));
        }
        if !bytes.is_empty() {
            queue.write_buffer(&held.buffer, 0, &bytes);
        }
        held.pipeline = Some(pipelines.pipeline(format));
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let held = self.0.borrow();
        let Some((held, pipeline)) = held
            .as_ref()
            .and_then(|held| Some((held, held.pipeline.as_ref()?)))
        else {
            return;
        };
        if held.drawn.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        for (texture, offset) in &held.drawn {
            pass.set_bind_group(0, texture, &[]);
            pass.set_bind_group(1, &held.group, &[*offset]);
            pass.draw(0..4, 0..1);
        }
    }
}

pub struct TextureDraw<K: 'static = ()> {
    texture: Rc<wgpu::Texture>,
    quad: Quad,
    _kept: K,
    bound: RefCell<Option<(u64, Bound)>>,
    quads: Quads,
}

impl TextureDraw {
    pub fn new(texture: impl Into<Rc<wgpu::Texture>>) -> Self {
        Self::keeping(texture, ())
    }
}

impl<K: 'static> TextureDraw<K> {
    pub fn keeping(texture: impl Into<Rc<wgpu::Texture>>, kept: K) -> Self {
        Self {
            texture: texture.into(),
            quad: Quad::default(),
            _kept: kept,
            bound: RefCell::new(None),
            quads: Quads::default(),
        }
    }

    pub fn with(mut self, quad: Quad) -> Self {
        self.quad = quad;
        self
    }
}

impl<K: 'static> Draw for TextureDraw<K> {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipelines = Pipelines::of(device, &at);
        let mut bound = self.bound.borrow_mut();
        if bound
            .as_ref()
            .is_none_or(|(renderer, _)| *renderer != at.renderer)
        {
            *bound = Some((at.renderer, pipelines.bind(&self.texture)));
        }
        let (_, texture) = bound.as_ref().expect("the texture was just bound");
        let quad = Quad {
            rect: at.rect,
            clip: at.clip,
            ..self.quad
        };
        self.quads.prepare(
            &pipelines,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            [(texture, quad)],
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
    }
}
