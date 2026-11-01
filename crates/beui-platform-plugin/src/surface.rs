use std::rc::Rc;

use beui_core::app::Setup;
use beui_core::context::{Moved, RendererInfo, WindowOutput};
use beui_core::display::Layer;
use beui_runner::renderer::Renderer;
use beui_types::Color32;
use beui_types::geometry::Region;
use beui_types::{Vec2, vec2};
use block_editor_plugin::driver::{Damage, Surface};

#[cfg(target_arch = "wasm32")]
use beui_renderer_wgpu::Repaint;
#[cfg(target_arch = "wasm32")]
use beui_types::{Pos2, Rect};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceGpu, SurfaceRect};

#[derive(Default)]
pub(crate) struct Painter {
    look: Option<(f32, Vec2, Color32)>,
    output: Option<WindowOutput>,
    #[cfg(target_arch = "wasm32")]
    renderer: Option<beui_renderer_wgpu::Renderer>,
}

impl Painter {
    pub(crate) fn output(&self) -> Option<&WindowOutput> {
        self.output.as_ref()
    }
}

pub(crate) fn note(surface: &mut Surface<Painter>, output: &WindowOutput, by: Vec2) {
    let output = shifted(output, by);
    let background = output.window.appearance.clear_color;
    let look = (output.pixels_per_point, by, background);
    let painter = &mut surface.painter;
    let moved = painter.look.as_ref() != Some(&look);
    painter.look = Some(look);
    let damage = match output.damaged() {
        _ if moved => Some(Damage::Everything),
        Some(region) => Some(Damage::Region(region)),
        None if output.changed => Some(Damage::Everything),
        None => None,
    };
    painter.output = Some(output);
    if let Some(damage) = damage {
        surface.damage(damage);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn paint(
    painter: &mut Painter,
    gpu: &SurfaceGpu,
    target: &PaintTarget<'_>,
    (width, height): (u32, u32),
    damage: Damage,
) -> Option<Vec<SurfaceRect>> {
    let output = painter.output.as_ref()?;
    let scale = output.pixels_per_point;
    let screen = vec2(width as f32, height as f32);
    let frame = Rect::from_min_size(Pos2::ZERO, screen / scale);
    let background = painter
        .look
        .map_or(Color32::TRANSPARENT, |(_, _, background)| background);
    let whole = Repaint::Region {
        region: Region::from(frame),
        background,
    };
    let repaint = match damage {
        Damage::Everything => whole,
        Damage::Region(region) => Repaint::Region {
            region: region.clipped(frame),
            background,
        },
    };
    if let Repaint::Region { region, .. } = repaint
        && region.is_empty()
    {
        return None;
    }
    let renderer = painter
        .renderer
        .get_or_insert_with(|| beui_renderer_wgpu::Renderer::new(&gpu.device, gpu.format));
    let prepared = renderer.prepare(&gpu.device, &gpu.queue, output, screen, scale, repaint);
    if matches!(repaint, Repaint::Region { .. }) && matches!(prepared, Repaint::Everything) {
        renderer.prepare(&gpu.device, &gpu.queue, output, screen, scale, whole);
    }
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        target.view,
        (target.width, target.height),
        wgpu::LoadOp::Load,
    );
    gpu.queue.submit([encoder.finish()]);
    Some(match renderer.scissors() {
        Some(scissors) => scissors
            .iter()
            .map(|[x, y, width, height]| SurfaceRect {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            })
            .collect(),
        None => target.whole(),
    })
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
