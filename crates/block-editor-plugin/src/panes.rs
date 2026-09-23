use block_plugin_api::{EditorInstanceId, ScreenLayout, ScreenPlacement};
use std::{collections::HashMap, time::Duration};

use crate::screens::Screens;

pub(crate) struct Panes {
    generation: Option<u64>,
    format: wgpu::TextureFormat,
    renderers: HashMap<EditorInstanceId, beui::Renderer>,
}

struct Target<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    view: &'a wgpu::TextureView,
    layout: &'a ScreenLayout,
}

pub(crate) struct Painted {
    pub(crate) repaint: Option<Duration>,
}

fn paint(
    renderer: &mut beui::Renderer,
    target: &Target<'_>,
    outputs: &[(ScreenPlacement, beui::FrameOutput)],
    instance: EditorInstanceId,
    cleared: &mut bool,
) {
    let layout = target.layout;
    let screen = beui::vec2(layout.width as f32, layout.height as f32);
    for (placement, output) in outputs
        .iter()
        .filter(|(placement, _)| placement.instance == instance)
    {
        let scale = output.pixels_per_point();
        let _ = renderer.prepare(
            target.device,
            target.queue,
            output,
            screen,
            scale,
            beui::Repaint::Everything,
        );
        let mut encoder = target
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plugin beui pane"),
                color_attachments: &[Some(attachment(target.view, *cleared))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            *cleared = true;
            pass.set_scissor_rect(
                placement.x.min(layout.width),
                placement.y.min(layout.height),
                placement
                    .width
                    .min(layout.width - placement.x.min(layout.width)),
                placement
                    .height
                    .min(layout.height - placement.y.min(layout.height)),
            );
            renderer.paint(&mut pass);
        }
        target.queue.submit([encoder.finish()]);
    }
}

fn attachment(view: &wgpu::TextureView, cleared: bool) -> wgpu::RenderPassColorAttachment<'_> {
    let load = match cleared {
        true => wgpu::LoadOp::Load,
        false => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
    };
    wgpu::RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: wgpu::Operations {
            load,
            store: wgpu::StoreOp::Store,
        },
        depth_slice: None,
    }
}

impl Panes {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            renderers: HashMap::new(),
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
        let mut outputs = Vec::new();
        for placement in &layout.screens {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let output = session.run(placement.region, layout.generation);
            changed |= output.changed;
            repaint = repaint.min(output.repaint_after);
            outputs.push((*placement, output));
        }
        self.renderers
            .retain(|instance, _| screens.is_open(*instance));
        let painted = Painted {
            repaint: (repaint < Duration::MAX).then_some(repaint),
        };
        if !changed {
            return painted;
        }
        let mut instances = Vec::new();
        for (placement, _) in &outputs {
            if !instances.contains(&placement.instance) {
                instances.push(placement.instance);
            }
        }
        let target = Target {
            device,
            queue,
            view,
            layout,
        };
        let mut cleared = false;
        let format = self.format;
        for instance in instances {
            let renderer = self
                .renderers
                .entry(instance)
                .or_insert_with(|| beui::Renderer::new(device, format));
            paint(renderer, &target, &outputs, instance, &mut cleared);
        }
        painted
    }
}
