use block_plugin_api::{EditorInstanceId, ScreenDamage, SurfaceRect};
use block_plugin_api::{ScreenLayout, ScreenPlacement};
use std::collections::HashSet;
use std::time::Duration;

use crate::plugin::PaintTarget;
use crate::screens::{Dirty, Screens};

#[derive(Default)]
pub(crate) struct Panes {
    generation: Option<u64>,
    repainting: HashSet<EditorInstanceId>,
}

pub(crate) struct Ran {
    pub(crate) repaint: Option<Duration>,
    pub(crate) painting: Vec<ScreenPlacement>,
}

impl Panes {
    pub(crate) fn run(
        &mut self,
        layout: &ScreenLayout,
        screens: &mut Screens,
        fresh: &HashSet<u32>,
    ) -> Ran {
        let mut repaint = Duration::MAX;
        let moved = self.generation != Some(layout.generation);
        self.generation = Some(layout.generation);
        let dirty = match moved {
            true => {
                screens.take_dirty();
                Dirty::Everything
            }
            false => screens.take_dirty(),
        };
        let repainting = std::mem::take(&mut self.repainting);
        let mut painting = Vec::new();
        for placement in &layout.screens {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let mut changed = fresh.contains(&placement.surface);
            if dirty.contains(placement.instance) || repainting.contains(&placement.instance) {
                let frame = session.run(placement.region, layout.generation);
                screens.ran(placement.instance);
                changed |= frame.changed;
                if let Some(after) = frame.repaint_after {
                    repaint = repaint.min(after);
                    self.repainting.insert(placement.instance);
                }
            }
            if changed {
                painting.push(*placement);
            }
        }
        Ran {
            repaint: (repaint < Duration::MAX).then_some(repaint),
            painting,
        }
    }

    pub(crate) fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        screens: &mut Screens,
        painting: Vec<ScreenPlacement>,
    ) -> Result<Vec<ScreenDamage>, String> {
        let mut damage = Vec::new();
        for placement in painting {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let texture = block_gpu_guest::acquire_surface_texture(placement.surface)?;
            let age = block_gpu_guest::surface_age(placement.surface);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            if age == 0 {
                clear(device, queue, &view);
            }
            let target = PaintTarget {
                device,
                queue,
                view: &view,
                format,
                width: placement.width,
                height: placement.height,
                region: placement.region,
                age,
            };
            let drawn = session.paint(&target);
            block_gpu_guest::present_surface(placement.surface);
            let drawn = match age {
                0 => target.whole(),
                _ => drawn,
            };
            damage.extend(drawn.into_iter().map(|rect: SurfaceRect| ScreenDamage {
                screen: placement.screen,
                rect,
            }));
        }
        Ok(damage)
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
