use std::cell::RefCell;
use std::rc::Rc;

use beui_renderer_wgpu::{Draw, DrawAt};
use block_editor_plugin::wgpu;
use block_plugin_api::PreviewImage;

const UNIFORM_BYTES: u64 = 48;
const KEPT_RENDERERS: usize = 4;

struct Pipeline {
    renderer: u64,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

struct Bound {
    renderer: u64,
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

thread_local! {
    static PIPELINES: RefCell<Vec<Rc<Pipeline>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct PictureDraw {
    image: PreviewImage,
    rotation: f32,
    opacity: f32,
    texture: Option<Rc<wgpu::Texture>>,
    bound: RefCell<Option<Bound>>,
}

impl PictureDraw {
    pub(crate) fn new(image: PreviewImage, rotation: f32, opacity: f32) -> Self {
        Self {
            image,
            rotation,
            opacity,
            texture: block_editor_plugin::preview_texture(image.image),
            bound: RefCell::new(None),
        }
    }
}

impl Draw for PictureDraw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let Some(texture) = self.texture.as_ref() else {
            return;
        };
        let pipeline = pipeline(device, at);
        let mut held = self.bound.borrow_mut();
        let bound = match held.take() {
            Some(bound) if bound.renderer == at.renderer => bound,
            _ => bind(device, &pipeline, texture),
        };
        let (sin, cos) = self.rotation.sin_cos();
        let values: [f32; 12] = [
            at.rect[0],
            at.rect[1],
            at.rect[2],
            at.rect[3],
            at.screen.x.max(1.0),
            at.screen.y.max(1.0),
            cos,
            sin,
            self.opacity.clamp(0.0, 1.0),
            f32::from(u8::from(at.format.is_srgb())),
            0.0,
            0.0,
        ];
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        queue.write_buffer(&bound.buffer, 0, &bytes);
        *held = Some(bound);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        if let Some(bound) = self.bound.borrow().as_ref() {
            pass.set_pipeline(&bound.pipeline);
            pass.set_bind_group(0, &bound.group, &[]);
            pass.draw(0..4, 0..1);
        }
    }
}

impl std::fmt::Debug for PictureDraw {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PictureDraw")
            .field("image", &self.image)
            .finish()
    }
}

fn bind(device: &wgpu::Device, pipeline: &Pipeline, texture: &wgpu::Texture) -> Bound {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("preview picture"),
        size: UNIFORM_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("preview picture"),
        layout: &pipeline.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
            },
        ],
    });
    Bound {
        renderer: pipeline.renderer,
        buffer,
        group,
        pipeline: pipeline.pipeline.clone(),
    }
}

fn pipeline(device: &wgpu::Device, at: DrawAt) -> Rc<Pipeline> {
    PIPELINES.with_borrow_mut(|pipelines| {
        if let Some(pipeline) = pipelines
            .iter()
            .find(|pipeline| pipeline.renderer == at.renderer)
        {
            return pipeline.clone();
        }
        let pipeline = Rc::new(create(device, at));
        if pipelines.len() == KEPT_RENDERERS {
            pipelines.remove(0);
        }
        pipelines.push(pipeline.clone());
        pipeline
    })
}

fn create(device: &wgpu::Device, at: DrawAt) -> Pipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("picture.wgsl"));
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("preview picture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
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
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("preview picture"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("preview picture"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
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
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: at.format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    Pipeline {
        renderer: at.renderer,
        pipeline,
        layout,
        sampler: device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("preview picture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }),
    }
}
