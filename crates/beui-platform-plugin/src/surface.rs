use std::collections::VecDeque;
use std::rc::Rc;

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{Moved, RendererInfo, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::geometry::{Vec2, vec2};
use beui_core::renderer::Renderer;
use block_plugin_api::SurfaceRect;

#[cfg(target_arch = "wasm32")]
use beui_core::geometry::{Pos2, Rect};
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
    pub(crate) size: (u32, u32),
    pub(crate) age: u32,
    pub(crate) visible: Vec2,
    look: Option<(f32, Vec2, Color32)>,
    pending: Option<Damage>,
    output: Option<WindowOutput>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    history: VecDeque<Option<Damage>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    drawn: Vec<SurfaceRect>,
    #[cfg(target_arch = "wasm32")]
    gpu: Option<Gpu>,
}

#[cfg(target_arch = "wasm32")]
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: beui_renderer_wgpu::Renderer,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            size: (0, 0),
            age: 0,
            visible: Vec2::ZERO,
            look: None,
            pending: None,
            output: None,
            history: VecDeque::new(),
            drawn: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            gpu: None,
        }
    }
}

impl Surface {
    pub(crate) fn unpainted(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn output(&self) -> Option<&WindowOutput> {
        self.output.as_ref()
    }

    pub(crate) fn note(&mut self, output: &WindowOutput) {
        let output = shifted(output, self.visible);
        let background = output.window.appearance.clear_color;
        let look = (output.pixels_per_point, self.visible, background);
        let moved = self.look.as_ref() != Some(&look);
        self.look = Some(look);
        let damage = match output.damaged() {
            _ if moved => Some(Damage::Everything),
            Some(region) => Some(Damage::Region(region)),
            None if output.changed => Some(Damage::Everything),
            None => None,
        };
        if let Some(damage) = damage {
            self.pending = Some(damage.union(self.pending));
        }
        self.output = Some(output);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn settle(&mut self) {
        self.pending = None;
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn settle(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn paint(
        &mut self,
        target: &block_editor_plugin::PaintTarget<'_>,
    ) -> Vec<SurfaceRect> {
        self.age = target.age;
        let painted = self.prepare();
        self.present(target, painted);
        std::mem::take(&mut self.drawn)
    }

    #[cfg(target_arch = "wasm32")]
    fn prepare(&mut self) -> Option<Option<Damage>> {
        let output = self.output.clone()?;
        let (width, height) = self.size;
        let scale = output.pixels_per_point;
        let screen = vec2(width as f32, height as f32);
        let frame = Rect::from_min_size(Pos2::ZERO, screen / scale);
        let painted = self.pending.take();
        let damage = self.repaint(painted)?;
        let background = self
            .look
            .map_or(Color32::TRANSPARENT, |(_, _, background)| background);
        let whole = Repaint::Region {
            region: Region::from(frame),
            background,
        };
        let repaint = match (self.age, damage) {
            (0, _) | (_, Damage::Everything) => whole,
            (_, Damage::Region(region)) => Repaint::Region {
                region: region.clipped(frame),
                background,
            },
        };
        if let Repaint::Region { region, .. } = repaint
            && region.is_empty()
        {
            return None;
        }
        let gpu = self.gpu()?;
        let prepared =
            gpu.renderer
                .prepare(&gpu.device, &gpu.queue, &output, screen, scale, repaint);
        if matches!(repaint, Repaint::Region { .. }) && matches!(prepared, Repaint::Everything) {
            gpu.renderer
                .prepare(&gpu.device, &gpu.queue, &output, screen, scale, whole);
        }
        Some(painted)
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
    fn present(
        &mut self,
        target: &block_editor_plugin::PaintTarget<'_>,
        painted: Option<Option<Damage>>,
    ) {
        self.drawn.clear();
        let gpu = self.gpu.as_mut();
        match (painted, gpu) {
            (Some(painted), Some(gpu)) => {
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                gpu.renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    target.view,
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
}

fn shifted(output: &WindowOutput, by: Vec2) -> WindowOutput {
    if by == Vec2::ZERO {
        return output.clone();
    }
    let layers = output
        .layers
        .iter()
        .map(|layer| match layer {
            Layer::Display(at, display) => Layer::Display(*at - by, Rc::clone(display)),
            layer => layer.clone(),
        })
        .collect();
    let mut damage = Region::NOTHING;
    for rect in output.damage.rects() {
        damage.add(rect.translate(-by));
    }
    WindowOutput {
        layers: Rc::new(layers),
        damage,
        moved: output.moved.map(|moved| Moved {
            from: moved.from.translate(-by),
            by: moved.by,
        }),
        ..output.clone()
    }
}

pub(crate) struct ModuleRenderer;

impl Renderer for ModuleRenderer {
    fn name(&self) -> &'static str {
        "plugin surfaces"
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

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(1.0, 1.0))
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        false
    }

    fn note(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) {}

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}
