use block_editor_plugin::be_block::TriangleContent;
use block_editor_plugin::wgpu;
use block_editor_plugin::{
    BlockParent, EditorHost, Frame, InputEvent, Instance, PaintTarget, Plugin, Region,
};
use uuid::Uuid;

pub struct TrianglePlugin;

impl Plugin for TrianglePlugin {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(Triangle::new(host))
    }
}

pub(crate) struct Triangle {
    host: EditorHost,
    painted: bool,
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

impl Triangle {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            host,
            painted: false,
            pipeline: None,
        }
    }
}

impl Instance for Triangle {
    fn connect(&mut self, _block_id: Uuid) {}

    fn create_block(&mut self) -> Result<Uuid, String> {
        Ok(self
            .host
            .blocks()
            .create(&TriangleContent::default(), BlockParent::Detached))
    }

    fn input(&mut self, _region: &Region, _event: &InputEvent) {}

    fn update(&mut self, region: &Region, _settings: Option<&mut Vec<u8>>) -> Frame {
        Frame {
            changed: !self.painted,
            painted: vec![region.rect],
            ..Frame::default()
        }
    }

    fn paint(&mut self, target: &PaintTarget<'_>) {
        let (x, y, width, height) = target.scissor();
        let side = width.min(height);
        if side == 0 {
            return;
        }
        let pipeline = match &self.pipeline {
            Some((format, pipeline)) if *format == target.format => pipeline,
            _ => {
                let pipeline = pipeline(target.device, target.format);
                &self.pipeline.insert((target.format, pipeline)).1
            }
        };
        let mut encoder = target
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("triangle"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
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
            pass.set_scissor_rect(x, y, width, height);
            pass.set_viewport(
                (x + (width - side) / 2) as f32,
                (y + (height - side) / 2) as f32,
                side as f32,
                side as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(pipeline);
            pass.draw(0..3, 0..1);
        }
        target.queue.submit([encoder.finish()]);
        self.painted = true;
    }
}

fn pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("triangle.wgsl"));
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("triangle"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("corner"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fill"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
