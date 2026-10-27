use std::cell::RefCell;
use std::rc::Rc;

use beui::{native::Draw, native::DrawAt};

const UNIFORM_BYTES: u64 = 48;
const KEPT_RENDERERS: usize = 4;

struct Pipeline {
    renderer: u64,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

struct Bound {
    renderer: u64,
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    shown: bool,
}

thread_local! {
    static PIPELINES: RefCell<Vec<Rc<Pipeline>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct TextureDraw<K: 'static> {
    texture: wgpu::Texture,
    _kept: K,
    bound: RefCell<Option<Bound>>,
}

impl<K: 'static> TextureDraw<K> {
    pub(crate) fn new(texture: wgpu::Texture, kept: K) -> Self {
        Self {
            texture,
            _kept: kept,
            bound: RefCell::new(None),
        }
    }
}

pub(crate) fn placed(at: &DrawAt) -> Option<([f32; 4], [f32; 4])> {
    let rect = at.rect;
    let clip = [
        at.clip[0].max(rect[0]),
        at.clip[1].max(rect[1]),
        at.clip[2].min(rect[2]),
        at.clip[3].min(rect[3]),
    ];
    let (width, height) = (rect[2] - rect[0], rect[3] - rect[1]);
    if clip[2] <= clip[0] || clip[3] <= clip[1] || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let u = |x: f32| (x - rect[0]) / width;
    let v = |y: f32| (y - rect[1]) / height;
    Some((clip, [u(clip[0]), v(clip[1]), u(clip[2]), v(clip[3])]))
}

impl<K: 'static> Draw for TextureDraw<K> {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipeline = pipeline(device, at);
        let mut held = self.bound.borrow_mut();
        let mut bound = match held.take() {
            Some(bound) if bound.renderer == at.renderer => bound,
            _ => bind(device, &pipeline, &self.texture),
        };
        bound.shown = false;
        if let Some((rect, uv)) = placed(&at) {
            let values: [f32; 12] = [
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                uv[0],
                uv[1],
                uv[2],
                uv[3],
                at.screen.x.max(1.0),
                at.screen.y.max(1.0),
                0.0,
                0.0,
            ];
            queue.write_buffer(&bound.buffer, 0, bytemuck::cast_slice(&values));
            bound.shown = true;
        }
        *held = Some(bound);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        if let Some(bound) = self.bound.borrow().as_ref().filter(|bound| bound.shown) {
            pass.set_pipeline(&bound.pipeline);
            pass.set_bind_group(0, &bound.group, &[]);
            pass.draw(0..4, 0..1);
        }
    }
}

fn bind(device: &wgpu::Device, pipeline: &Pipeline, texture: &wgpu::Texture) -> Bound {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shown texture"),
        size: UNIFORM_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shown texture"),
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
        shown: false,
    }
}

fn pipeline(device: &wgpu::Device, at: DrawAt) -> Rc<Pipeline> {
    PIPELINES.with_borrow_mut(|pipelines| {
        if let Some(pipeline) = pipelines
            .iter()
            .find(|pipeline| pipeline.renderer == at.renderer && pipeline.format == at.format)
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
    let shader = device.create_shader_module(wgpu::include_wgsl!("texture_draw.wgsl"));
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shown texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
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
        label: Some("shown texture"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shown texture"),
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
        format: at.format,
        pipeline,
        layout,
        sampler: device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shown texture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }),
    }
}

#[cfg(test)]
mod tests;
