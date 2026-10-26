use block_plugin_api::{EditorInstanceId, EditorRegion, ScreenDamage, SurfaceRect};
use block_plugin_api::{ScreenLayout, ScreenPlacement};
use std::collections::HashSet;
use std::time::Duration;

use crate::cooldown::Cooldown;

use crate::plugin::PaintTarget;
use crate::screens::Screens;

#[derive(Default)]
pub(crate) struct Panes {
    repainting: HashSet<EditorInstanceId>,
    unpainted: HashSet<EditorInstanceId>,
    previews: Cooldown,
    settling: HashSet<u32>,
}

pub(crate) struct Ran {
    pub(crate) repaint: Option<Duration>,
    pub(crate) animating: bool,
    pub(crate) painting: Vec<ScreenPlacement>,
}

impl Panes {
    pub(crate) fn run(
        &mut self,
        layout: &ScreenLayout,
        screens: &mut Screens,
        fresh: &HashSet<u32>,
        seen: bool,
        rendered: bool,
    ) -> Ran {
        let mut repaint = Duration::MAX;
        let mut animating = false;
        let dirty = screens.take_dirty();
        let repainting = std::mem::take(&mut self.repainting);
        let unpainted = match seen {
            true => std::mem::take(&mut self.unpainted),
            false => HashSet::new(),
        };
        let mut painting = Vec::new();
        let now = crate::clock::frame_now();
        let settling = std::mem::take(&mut self.settling);
        self.previews.keep(|surface| {
            layout
                .screens
                .iter()
                .any(|placement| placement.surface == surface)
        });
        let windows_last = layout
            .screens
            .iter()
            .filter(|placement| !matches!(placement.region, EditorRegion::Window(_)))
            .chain(
                layout
                    .screens
                    .iter()
                    .filter(|placement| matches!(placement.region, EditorRegion::Window(_))),
            );
        for placement in windows_last {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let mut changed = fresh.contains(&placement.surface);
            let due = changed
                || dirty.contains(placement.instance)
                || repainting.contains(&placement.instance)
                || unpainted.contains(&placement.instance);
            let held = due && !changed && !settling.contains(&placement.surface);
            if let Some(wait) = self.cooling(placement, now).filter(|_| held) {
                repaint = repaint.min(wait);
                self.repainting.insert(placement.instance);
                continue;
            }
            if due {
                let age = match rendered {
                    true => block_gpu_guest::surface_age(placement.surface),
                    false => 0,
                };
                let frame = session.run(placement.region, age);
                screens.ran(placement.instance);
                changed |= frame.changed;
                animating |= frame.animating;
                if let Some(after) = frame.repaint_after {
                    repaint = repaint.min(after);
                    self.repainting.insert(placement.instance);
                    if after.is_zero() && !frame.animating {
                        self.settling.insert(placement.surface);
                    }
                }
            }
            let shown = seen || placement.region == EditorRegion::Preview;
            match (changed, shown) {
                (true, true) => {
                    if placement.region == EditorRegion::Preview {
                        let fresh = fresh.contains(&placement.surface);
                        self.previews.painted(placement.surface, now, fresh);
                    }
                    painting.push(*placement);
                }
                (true, false) => {
                    self.unpainted.insert(placement.instance);
                }
                (false, _) => {}
            }
        }
        Ran {
            repaint: (repaint < Duration::MAX).then_some(repaint),
            animating,
            painting,
        }
    }

    fn cooling(&self, placement: &ScreenPlacement, now: Duration) -> Option<Duration> {
        match placement.region {
            EditorRegion::Preview => self.previews.wait(placement.surface, now),
            EditorRegion::Frame | EditorRegion::ArtifactSettings | EditorRegion::Window(_) => None,
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
