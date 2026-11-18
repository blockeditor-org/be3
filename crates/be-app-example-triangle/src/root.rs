use be_sdk::wgpu;
use std::collections::HashMap;

use be_sdk::{
    App, BlockParent, EditorHost, Frame, Input, Instance, PaintTarget, ShownView, SurfaceRect,
    TimedInput, View, ViewHandler, ViewRef,
};
use be_type_example_triangle::TriangleContent;
use uuid::Uuid;

pub struct TriangleApp;

impl App for TriangleApp {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(Triangle::new(host))
    }
}

pub(crate) struct Triangle {
    host: EditorHost,
}

impl Triangle {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self { host }
    }
}

#[derive(Default)]
pub(crate) struct TriangleViews {
    painted: HashMap<ViewRef, bool>,
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    /// What input each view heard, which tints its background (`Heard::tint`), so a host's
    /// test sees its input reach the triangle.
    heard: HashMap<ViewRef, Heard>,
}

/// What input a view heard: a key, a pointer's press, a touch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Heard {
    pub(crate) key: bool,
    pub(crate) press: bool,
    pub(crate) touch: bool,
}

impl Heard {
    /// The background a view that heard this is cleared to: red for a key, green for a press,
    /// blue for a touch, added together; none, leaving it clear, for a view that heard none.
    pub(crate) fn tint(self) -> Option<wgpu::Color> {
        let channel = |heard: bool| if heard { 0.8 } else { 0.0 };
        (self != Self::default()).then(|| wgpu::Color {
            r: channel(self.key),
            g: channel(self.press),
            b: channel(self.touch),
            a: 1.0,
        })
    }
}

impl Instance for Triangle {
    fn connect(&mut self, _block_id: Uuid) {}

    fn create_block(&mut self) -> Result<Uuid, String> {
        Ok(self
            .host
            .graph()
            .create(&TriangleContent::default(), BlockParent::Detached))
    }

    fn open_view(&mut self, _view: View) {
        if !self.host.handles_views() {
            self.host.handle_views(Box::new(TriangleViews::default()));
        }
    }
}

impl ViewHandler for TriangleViews {
    fn input(&mut self, region: &ShownView, event: &TimedInput, _over: Option<ViewRef>) {
        let heard = self.heard.entry(region.id()).or_default();
        let before = *heard;
        match event.input {
            Input::Key(_) => heard.key = true,
            Input::PointerButton { pressed: true, .. } => heard.press = true,
            Input::Touch { .. } => heard.touch = true,
            _ => {}
        }
        if *heard != before {
            self.painted.insert(region.id(), false);
        }
    }

    fn frame(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        self.painted
            .retain(|id, _| regions.iter().any(|region| region.id() == *id));
        self.heard
            .retain(|id, _| regions.iter().any(|region| region.id() == *id));
        regions
            .iter()
            .map(|region| Frame {
                changed: !self.painted.get(&region.id()).copied().unwrap_or_default(),
                painted: vec![region.rect],
                ..Frame::default()
            })
            .collect()
    }

    fn paint(&mut self, region: ViewRef, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        let (width, height) = (target.width, target.height);
        let side = width.min(height);
        if side == 0 {
            return Vec::new();
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
                        load: match self.heard.get(&region).and_then(|heard| heard.tint()) {
                            Some(tint) => wgpu::LoadOp::Clear(tint),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                ((width - side) / 2) as f32,
                ((height - side) / 2) as f32,
                side as f32,
                side as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(pipeline);
            pass.draw(0..3, 0..1);
        }
        target.queue.submit([encoder.finish()]);
        self.painted.insert(region, true);
        target.whole()
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
