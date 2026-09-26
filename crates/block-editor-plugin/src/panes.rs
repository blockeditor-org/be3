use block_plugin_api::{ScreenLayout, ScreenPlacement};
use std::time::Duration;

use crate::plugin::PaintTarget;
use crate::screens::Screens;

pub(crate) struct Panes {
    generation: Option<u64>,
    format: wgpu::TextureFormat,
}

pub(crate) struct Painted {
    pub(crate) repaint: Option<Duration>,
}

impl Panes {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            generation: None,
        }
    }

    pub(crate) fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        layout: &ScreenLayout,
        screens: &mut Screens,
    ) -> Painted {
        let mut repaint = Duration::MAX;
        let mut changed = self.generation != Some(layout.generation);
        self.generation = Some(layout.generation);
        let mut placed: Vec<ScreenPlacement> = Vec::new();
        for placement in &layout.screens {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let frame = session.run(placement.region, layout.generation);
            changed |= frame.changed;
            repaint = repaint.min(frame.repaint_after.unwrap_or(Duration::MAX));
            placed.push(*placement);
        }
        let painted = Painted {
            repaint: (repaint < Duration::MAX).then_some(repaint),
        };
        if !changed {
            return painted;
        }
        clear(device, queue, view);
        for placement in placed {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            session.paint(&PaintTarget {
                device,
                queue,
                view,
                format: self.format,
                width: layout.width,
                height: layout.height,
                placement,
            });
        }
        painted
    }
}

fn clear(device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("plugin surface clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    queue.submit([encoder.finish()]);
}
