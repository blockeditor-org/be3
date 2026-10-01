use block_plugin_api::{ScreenLayout, ScreenPlacement, SurfaceRect};
use block_plugin_api::EditorInstanceId;
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use crate::plugin::PaintTarget;
use crate::screens::{Dirty, Screens};

pub(crate) struct Panes {
    generation: Option<u64>,
    format: wgpu::TextureFormat,
    presented: VecDeque<u64>,
    repainting: HashSet<EditorInstanceId>,
}

const REMEMBERED_PRESENTS: usize = 4;

pub(crate) struct Ran {
    pub(crate) changed: bool,
    pub(crate) repaint: Option<Duration>,
    placed: Vec<ScreenPlacement>,
}

impl Panes {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            generation: None,
            presented: VecDeque::new(),
            repainting: HashSet::new(),
        }
    }

    pub(crate) fn run(&mut self, layout: &ScreenLayout, screens: &mut Screens) -> Ran {
        let mut repaint = Duration::MAX;
        let mut changed = self.generation != Some(layout.generation);
        self.generation = Some(layout.generation);
        let dirty = match changed {
            true => {
                screens.take_dirty();
                Dirty::Everything
            }
            false => screens.take_dirty(),
        };
        let repainting = std::mem::take(&mut self.repainting);
        let mut placed: Vec<ScreenPlacement> = Vec::new();
        for placement in &layout.screens {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            placed.push(*placement);
            if !dirty.contains(placement.instance) && !repainting.contains(&placement.instance) {
                continue;
            }
            let frame = session.run(placement.region, layout.generation);
            screens.ran(placement.instance);
            changed |= frame.changed;
            if let Some(after) = frame.repaint_after {
                repaint = repaint.min(after);
                self.repainting.insert(placement.instance);
            }
        }
        Ran {
            changed,
            repaint: (repaint < Duration::MAX).then_some(repaint),
            placed,
        }
    }

    pub(crate) fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        layout: &ScreenLayout,
        screens: &mut Screens,
        ran: Ran,
        age: u32,
    ) -> Vec<SurfaceRect> {
        let kept = age > 0 && self.presented.get(age as usize - 1) == Some(&layout.generation);
        let age = if kept { age } else { 0 };
        if age == 0 {
            clear(device, queue, view);
        }
        self.presented.push_front(layout.generation);
        self.presented.truncate(REMEMBERED_PRESENTS);
        let mut damage = Vec::new();
        for placement in ran.placed {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            damage.extend(session.paint(&PaintTarget {
                device,
                queue,
                view,
                format: self.format,
                width: layout.width,
                height: layout.height,
                placement,
                age,
            }));
        }
        match age {
            0 => vec![SurfaceRect {
                x: 0,
                y: 0,
                width: layout.width,
                height: layout.height,
            }],
            _ => damage,
        }
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
