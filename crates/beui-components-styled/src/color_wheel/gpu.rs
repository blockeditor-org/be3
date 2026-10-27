use std::cell::RefCell;
use std::rc::Rc;

use beui_view::drawing::Drawing;
use beui_renderer_wgpu::{Draw, DrawAt};
use bytemuck::{Pod, Zeroable};

use super::{Face, Model};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    rect: [f32; 4],
    clip: [f32; 4],
    screen: [f32; 2],
    hue: f32,
    model: f32,
    ring_width: f32,
    ring_gap: f32,
    gamut_line: f32,
    srgb: f32,
}

struct Pipeline {
    renderer: u64,
    device: wgpu::Device,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

struct Bound {
    renderer: u64,
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

const KEPT_RENDERERS: usize = 4;

thread_local! {
    static PIPELINES: RefCell<Vec<Rc<Pipeline>>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Default)]
pub(super) struct Slot(Rc<RefCell<Option<Bound>>>);

impl Slot {
    pub(super) fn drawing(&self, face: Face) -> Drawing {
        beui_renderer_wgpu::drawing(WheelDraw {
            slot: self.clone(),
            face,
        })
    }
}

struct WheelDraw {
    slot: Slot,
    face: Face,
}

impl Draw for WheelDraw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipeline = pipeline(device, at);
        let mut held = self.slot.0.borrow_mut();
        let bound = match held.take() {
            Some(bound) if bound.renderer == at.renderer => bound,
            _ => bind(&pipeline),
        };
        let scale = at.pixels_per_point;
        let uniform = Uniform {
            rect: at.rect,
            clip: at.clip,
            screen: [at.screen.x, at.screen.y],
            hue: self.face.hue,
            model: match self.face.model {
                Model::Hsv => 0.0,
                Model::Oklch => 1.0,
            },
            ring_width: self.face.ring_width * scale,
            ring_gap: self.face.ring_gap * scale,
            gamut_line: self.face.gamut_line * scale,
            srgb: f32::from(u8::from(at.format.is_srgb())),
        };
        queue.write_buffer(&bound.buffer, 0, bytemuck::bytes_of(&uniform));
        *held = Some(bound);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        if let Some(bound) = self.slot.0.borrow().as_ref() {
            pass.set_pipeline(&bound.pipeline);
            pass.set_bind_group(0, &bound.group, &[]);
            pass.draw(0..4, 0..1);
        }
    }
}

fn bind(pipeline: &Pipeline) -> Bound {
    let buffer = pipeline.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("color wheel"),
        size: std::mem::size_of::<Uniform>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = pipeline
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("color wheel"),
            layout: &pipeline.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
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
    let shader = device.create_shader_module(wgpu::include_wgsl!("wheel.wgsl"));
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("color wheel"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("color wheel"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("color wheel"),
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    Pipeline {
        renderer: at.renderer,
        device: device.clone(),
        pipeline,
        layout,
    }
}
