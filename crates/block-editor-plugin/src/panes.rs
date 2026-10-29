use block_plugin_api::{EditorInstanceId, ScreenDamage, SurfaceRect, ViewId};
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShownAnchored {
    pub(crate) placement: ScreenPlacement,
    pub(crate) anchored: crate::editor_session::AnchoredSurface,
    pub(crate) fresh: bool,
}

pub(crate) struct Ran {
    pub(crate) repaint: Option<Duration>,
    pub(crate) animating: bool,
    pub(crate) painting: Vec<ScreenPlacement>,
    pub(crate) opened: bool,
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
        let due = layout.screens.iter().any(|placement| {
            fresh.contains(&placement.surface)
                || dirty.contains(placement.instance)
                || repainting.contains(&placement.instance)
                || unpainted.contains(&placement.instance)
        });
        if !due {
            self.repainting = repainting;
            self.unpainted.extend(unpainted);
            return Ran {
                repaint: None,
                animating,
                painting,
                opened: false,
            };
        }
        let shown: Vec<(ScreenPlacement, u32)> = layout
            .screens
            .iter()
            .map(|placement| {
                let age = match rendered {
                    true => block_gpu_guest::surface_age(placement.surface),
                    false => 0,
                };
                (*placement, age)
            })
            .collect();
        let frames = screens.frame(&shown);
        for (placement, frame) in layout.screens.iter().zip(frames) {
            if screens.session(placement.instance).is_none() {
                continue;
            }
            let fresh = fresh.contains(&placement.surface);
            let changed = fresh || frame.changed;
            animating |= frame.animating;
            if let Some(after) = frame.repaint_after {
                repaint = repaint.min(after);
                self.repainting.insert(placement.instance);
                if after.is_zero() && !frame.animating {
                    self.settling.insert(placement.surface);
                }
            }
            let held = changed && !fresh && !settling.contains(&placement.surface);
            if let Some(wait) = self.cooling(placement, now).filter(|_| held) {
                repaint = repaint.min(wait);
                self.repainting.insert(placement.instance);
                continue;
            }
            let shown = seen || placement.view == ViewId::PREVIEW;
            match (changed, shown) {
                (true, true) => {
                    if placement.view == ViewId::PREVIEW {
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
        let opened: Vec<ScreenPlacement> = screens
            .take_opened()
            .into_iter()
            .map(|(placement, _)| placement)
            .collect();
        if seen {
            painting.extend(opened.iter().copied());
        }
        Ran {
            repaint: (repaint < Duration::MAX).then_some(repaint),
            animating,
            painting,
            opened: !opened.is_empty(),
        }
    }

    fn cooling(&self, placement: &ScreenPlacement, now: Duration) -> Option<Duration> {
        match placement.view {
            ViewId::PREVIEW => self.previews.wait(placement.surface, now),
            _ => None,
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
                age,
            };
            let drawn = session.paint(placement.view, &target);
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

impl Panes {
    pub(crate) fn paint_anchored(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        screens: &mut Screens,
        anchored: &[ShownAnchored],
    ) -> Result<(), String> {
        for shown in anchored {
            let Some(session) = screens.session(shown.placement.instance) else {
                continue;
            };
            let surface = shown.anchored.surface;
            let texture = block_gpu_guest::acquire_surface_texture(surface)?;
            let age = block_gpu_guest::surface_age(surface);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            if age == 0 {
                clear(device, queue, &view);
            }
            let [width, height] = shown.anchored.pixels;
            let target = PaintTarget {
                device,
                queue,
                view: &view,
                format,
                width,
                height,
                age,
            };
            session.paint_anchored(shown.placement.view, shown.anchored.view, &target);
            block_gpu_guest::present_surface(surface);
        }
        Ok(())
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
