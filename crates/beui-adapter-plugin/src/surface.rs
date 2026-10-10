use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::damage::Region;
use beui_core::filter::Filter;
use beui_core::geometry::{Rect, Vec2, vec2};
use beui_core::renderer::Renderer;
use beui_core::screens::Screen;
use block_plugin_api::SurfaceRect;

#[cfg(target_arch = "wasm32")]
use beui_core::geometry::Pos2;
#[cfg(target_arch = "wasm32")]
use beui_renderer_wgpu::Repaint;

#[cfg(target_arch = "wasm32")]
const REMEMBERED_PAINTS: usize = 4;

#[derive(Clone, Copy)]
enum Damage {
    Region(Region),
    Everything,
}

impl Damage {
    fn union(self, other: Option<Self>) -> Self {
        match (self, other) {
            (Self::Region(region), Some(Self::Region(added))) => Self::Region(region.union(added)),
            (damage, None) => damage,
            _ => Self::Everything,
        }
    }
}

pub(crate) struct Surface {
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) age: u32,
    pub(crate) laid: Rect,
    pub(crate) screens: Vec<Screen>,
    look: Option<(Option<Filter>, f32, Rect)>,
    pending: Option<Damage>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    history: VecDeque<Option<Damage>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    prepared: Option<Option<Damage>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    drawn: Vec<SurfaceRect>,
    #[cfg(target_arch = "wasm32")]
    gpu: Option<Gpu>,
    #[cfg(target_arch = "wasm32")]
    target: Option<Target>,
}

#[cfg(target_arch = "wasm32")]
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: beui_renderer_wgpu::Renderer,
}

#[cfg(target_arch = "wasm32")]
struct Target {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            size: None,
            age: 0,
            laid: Rect::NOTHING,
            screens: Vec::new(),
            look: None,
            pending: None,
            history: VecDeque::new(),
            prepared: None,
            drawn: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            gpu: None,
            #[cfg(target_arch = "wasm32")]
            target: None,
        }
    }
}

impl Surface {
    fn note(&mut self, output: &FrameOutput, scale: f32) {
        let look = (output.filter(), scale, self.laid);
        let moved = self.look != Some(look);
        self.look = Some(look);
        let damage = match output.damaged() {
            _ if moved => Damage::Everything,
            Some(region) => Damage::Region(region),
            None if output.changed => Damage::Everything,
            None => return,
        };
        self.pending = Some(damage.union(self.pending));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn prepare(&mut self, _output: &FrameOutput, _scale: f32) -> bool {
        self.pending.take().is_some()
    }

    #[cfg(target_arch = "wasm32")]
    fn prepare(&mut self, output: &FrameOutput, scale: f32) -> bool {
        let Some((width, height)) = self.size else {
            return false;
        };
        let screen = vec2(width as f32, height as f32);
        let frame = Rect::from_min_size(Pos2::ZERO, screen / scale);
        let painted = self.pending.take();
        let Some(damage) = self.repaint(painted) else {
            return false;
        };
        let whole = Repaint::Region {
            region: Region::from(frame),
            background: Color32::TRANSPARENT,
        };
        let repaint = match (self.age, damage) {
            (0, _) => Repaint::Everything,
            (_, Damage::Everything) => whole,
            (_, Damage::Region(region)) => Repaint::Region {
                region: region.clipped(frame),
                background: Color32::TRANSPARENT,
            },
        };
        if let Repaint::Region { region, .. } = repaint
            && region.is_empty()
        {
            return false;
        }
        let Some(gpu) = self.gpu() else {
            return false;
        };
        let prepared =
            gpu.renderer
                .prepare(&gpu.device, &gpu.queue, output, screen, scale, repaint);
        if matches!(repaint, Repaint::Region { .. }) && matches!(prepared, Repaint::Everything) {
            gpu.renderer
                .prepare(&gpu.device, &gpu.queue, output, screen, scale, whole);
        }
        self.prepared = Some(painted);
        true
    }

    #[cfg(target_arch = "wasm32")]
    fn gpu(&mut self) -> Option<&mut Gpu> {
        if self.gpu.is_none() {
            let gpu = block_editor_plugin::surface_gpu()?;
            self.gpu = Some(Gpu {
                renderer: beui_renderer_wgpu::Renderer::new(&gpu.device, gpu.format),
                device: gpu.device,
                queue: gpu.queue,
            });
        }
        self.gpu.as_mut()
    }

    #[cfg(target_arch = "wasm32")]
    fn repaint(&self, painted: Option<Damage>) -> Option<Damage> {
        let older = self
            .age
            .checked_sub(1)
            .map(|older| older as usize)
            .filter(|older| *older <= self.history.len());
        match older {
            Some(older) => self
                .history
                .iter()
                .take(older)
                .fold(painted, |repaint, damage| match (repaint, damage) {
                    (None, damage) => *damage,
                    (Some(repaint), damage) => Some(repaint.union(*damage)),
                }),
            None => Some(Damage::Everything),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn aim(&mut self, target: &block_editor_plugin::PaintTarget<'_>) {
        self.target = Some(Target {
            view: target.view.clone(),
            width: target.width,
            height: target.height,
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn present(&mut self) {
        let target = self.target.take();
        let painted = self.prepared.take();
        self.drawn.clear();
        let gpu = self.gpu.as_mut();
        match (painted, target, gpu) {
            (Some(painted), Some(target), Some(gpu)) => {
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                gpu.renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    &target.view,
                    (target.width, target.height),
                    wgpu::LoadOp::Load,
                );
                gpu.queue.submit([encoder.finish()]);
                self.drawn = match gpu.renderer.scissors() {
                    Some(scissors) => scissors
                        .iter()
                        .map(|[x, y, width, height]| SurfaceRect {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        })
                        .collect(),
                    None => vec![SurfaceRect {
                        x: 0,
                        y: 0,
                        width: target.width,
                        height: target.height,
                    }],
                };
                self.history.push_front(painted);
            }
            _ => self.history.push_front(None),
        }
        self.history.truncate(REMEMBERED_PAINTS);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_drawn(&mut self) -> Vec<SurfaceRect> {
        std::mem::take(&mut self.drawn)
    }
}

pub(crate) struct SurfaceRenderer(Rc<RefCell<Surface>>);

impl SurfaceRenderer {
    pub(crate) fn new(surface: Rc<RefCell<Surface>>) -> Self {
        Self(surface)
    }
}

impl Renderer for SurfaceRenderer {
    fn name(&self) -> &'static str {
        "plugin surface"
    }

    fn info(&self) -> RendererInfo {
        #[cfg(target_arch = "wasm32")]
        if let Some(gpu) = block_editor_plugin::surface_gpu() {
            return RendererInfo {
                rows: vec![("Surface format", format!("{:?}", gpu.format))],
            };
        }
        RendererInfo { rows: Vec::new() }
    }

    fn provide(&self, _setup: &mut Setup) {
        #[cfg(target_arch = "wasm32")]
        if let Some(gpu) = block_editor_plugin::surface_gpu() {
            _setup.provide(beui_renderer_wgpu::present::GpuSetup {
                device: gpu.device,
                queue: gpu.queue,
                format: gpu.format,
            });
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.0.borrow_mut().size = Some((width, height));
    }

    fn physical(&self) -> Option<Vec2> {
        let (width, height) = self.0.borrow().size?;
        Some(vec2(width as f32, height as f32))
    }

    fn screens(&self) -> Option<Vec<Screen>> {
        Some(self.0.borrow().screens.clone())
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, _background: Color32) -> bool {
        let mut surface = self.0.borrow_mut();
        surface.note(output, scale);
        surface.prepare(output, scale)
    }

    fn present(&mut self, _background: Color32) -> bool {
        #[cfg(target_arch = "wasm32")]
        self.0.borrow_mut().present();
        false
    }
}
