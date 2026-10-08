use std::cell::RefCell;
use std::rc::Rc;

use beui::{
    Color32, CursorIcon, FrameOutput, GpuSetup, Pos2, Rect, RendererInfo, Repainting, Setup, Vec2,
    vec2,
};
use beui_core::renderer::Renderer;

use crate::arrow;
use crate::gpu::{Gpu, SoftwareCursor, Sprite};
use crate::layout::bounds;
use crate::output::Output;
use crate::screen::{FORMAT, Frame, Pointer};

pub struct Displays {
    pub outputs: Vec<Output>,
    pub pointer: Pos2,
    pub scale: f32,
    pub icon: CursorIcon,
    pub gpu: Rc<Gpu>,
    cursor: SoftwareCursor,
    arrow: Sprite,
}

impl Displays {
    pub fn new(gpu: Rc<Gpu>, cursor: SoftwareCursor, scale: f32) -> Self {
        let arrow = gpu.rgba(arrow::WIDTH, arrow::HEIGHT, &arrow::pixels());
        Self {
            outputs: Vec::new(),
            pointer: Pos2::ZERO,
            scale,
            icon: CursorIcon::Default,
            gpu,
            cursor,
            arrow,
        }
    }

    pub fn rects(&self) -> Vec<Rect> {
        self.outputs
            .iter()
            .map(|output| output.screen.rect)
            .collect()
    }

    pub fn moved_pointer(&mut self) {
        for output in &mut self.outputs {
            output.screen.cursor = true;
        }
    }

    fn prepare(&mut self, frame: &FrameOutput, scale: f32, background: Color32) -> bool {
        let repaint = frame.changed.then(|| frame.repaint(background));
        for output in &mut self.outputs {
            if let Some(repaint) = repaint {
                output.screen.damage(repaint);
            }
            output.screen.prepare(&self.gpu, frame, scale);
        }
        self.outputs.iter().any(Output::wants_frame)
    }

    fn present(&mut self, background: Color32) {
        let gpu = Rc::clone(&self.gpu);
        let client = self.cursor.with(|image| {
            image.map(|image| {
                (
                    gpu.sprite(&image.texture, image.opaque),
                    image.size,
                    image.hotspot,
                )
            })
        });
        for output in &mut self.outputs {
            if !output.wants_frame() {
                continue;
            }
            let sprite = match (&client, self.icon) {
                (Some((sprite, size, hotspot)), _) => Pointer::Sprite {
                    sprite,
                    size: *size,
                    hotspot: *hotspot,
                },
                (None, CursorIcon::None) => Pointer::Hidden,
                (None, _) => Pointer::Sprite {
                    sprite: &self.arrow,
                    size: vec2(arrow::WIDTH as f32, arrow::HEIGHT as f32),
                    hotspot: Vec2::ZERO,
                },
            };
            let drawn = output.render(
                &gpu,
                Frame {
                    scale: self.scale,
                    clear: background,
                    pointer: self.pointer,
                    sprite,
                },
            );
            if let Err(error) = drawn {
                eprintln!("beui: an output could not be drawn: {error}");
            }
        }
    }
}

#[derive(Clone)]
pub struct Screens(Rc<RefCell<Displays>>);

impl Screens {
    pub fn rects(&self) -> Vec<Rect> {
        self.0
            .try_borrow()
            .map(|displays| displays.rects())
            .unwrap_or_default()
    }
}

pub struct DisplayRenderer(pub Rc<RefCell<Displays>>);

impl Renderer for DisplayRenderer {
    fn name(&self) -> &'static str {
        "drm"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![("Surface format", format!("{FORMAT:?}"))],
        }
    }

    fn provide(&self, setup: &mut Setup) {
        let displays = self.0.borrow();
        setup.provide(GpuSetup {
            device: displays.gpu.device().clone(),
            queue: displays.gpu.queue().clone(),
            format: FORMAT,
        });
        setup.provide(displays.cursor.clone());
        setup.provide(Screens(self.0.clone()));
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        let displays = self.0.borrow();
        if displays.outputs.is_empty() {
            return None;
        }
        Some(bounds(&displays.rects()).max.to_vec2() * displays.scale)
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool {
        self.0.borrow_mut().prepare(output, scale, background)
    }

    fn present(&mut self, background: Color32) -> bool {
        self.0.borrow_mut().present(background);
        false
    }
}
