use std::cell::RefCell;
use std::rc::Rc;

use beui::{
    Color32, CursorIcon, GpuSetup, Pos2, Rect, RendererInfo, Repainting, Screen, Setup, Vec2,
    WindowOutput,
};
use beui_core::renderer::Renderer;

use crate::cursors::{Cursors, Shape};
use crate::flips::Flips;
use crate::gpu::{Gpu, SoftwareCursor};
use crate::layout::bounds;
use crate::output::Output;
use crate::screen::{FORMAT, Frame, Pointer};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Icon(CursorIcon),
    Client {
        revision: u64,
        size: Vec2,
        hotspot: Vec2,
    },
}

pub struct Displays {
    pub outputs: Vec<Output>,
    pub pointer: Pos2,
    pub scale: f32,
    pub icon: CursorIcon,
    pub gpu: Rc<Gpu>,
    pub flips: Flips,
    cursor: SoftwareCursor,
    cursors: Cursors,
    look: Look,
    placed: Option<Rect>,
}

impl Displays {
    pub fn new(gpu: Rc<Gpu>, cursor: SoftwareCursor, scale: f32) -> Self {
        Self {
            outputs: Vec::new(),
            flips: Flips::default(),
            pointer: Pos2::ZERO,
            scale,
            icon: CursorIcon::Default,
            gpu,
            cursor,
            cursors: Cursors::new(Theme::from_env(), scale),
            look: Look::Icon(CursorIcon::Default),
            placed: None,
        }
    }

    pub fn rects(&self) -> Vec<Rect> {
        self.outputs
            .iter()
            .map(|output| output.screen.rect)
            .collect()
    }

    pub fn moved_pointer(&mut self) {
        self.replace_pointer();
    }

    fn look(&self) -> Look {
        self.cursor
            .with(|image| {
                image.map(|image| Look::Client {
                    revision: image.revision,
                    size: image.size,
                    hotspot: image.hotspot,
                })
            })
            .unwrap_or(Look::Icon(self.icon))
    }

    fn shape(&mut self, icon: CursorIcon) -> Option<Rc<Shape>> {
        self.cursors.get(&self.gpu, icon)
    }

    fn extent(&mut self, look: Look) -> Option<(Vec2, Vec2)> {
        match look {
            Look::Client { size, hotspot, .. } => Some((size, hotspot)),
            Look::Icon(icon) => self.shape(icon).map(|shape| (shape.size, shape.hotspot)),
        }
    }

    fn replace_pointer(&mut self) {
        self.look = self.look();
        let placed = self
            .extent(self.look)
            .map(|(size, hotspot)| Rect::from_min_size(self.pointer - hotspot, size).expand(1.0));
        for output in &mut self.outputs {
            let rect = output.screen.rect;
            if [self.placed, placed]
                .into_iter()
                .flatten()
                .any(|cursor| cursor.intersects(rect))
            {
                output.screen.cursor = true;
            }
        }
        self.placed = placed;
    }

    fn prepare(&mut self, frame: &WindowOutput, scale: f32, background: Color32) -> bool {
        if self.look() != self.look {
            self.replace_pointer();
        }
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
        let themed = match client {
            Some(_) => None,
            None => self.shape(self.icon),
        };
        for output in &mut self.outputs {
            if !output.wants_frame() {
                continue;
            }
            let sprite = match (&client, &themed) {
                (Some((sprite, size, hotspot)), _) => Pointer::Sprite {
                    sprite,
                    size: *size,
                    hotspot: *hotspot,
                },
                (None, Some(shape)) => Pointer::Sprite {
                    sprite: &shape.sprite,
                    size: shape.size,
                    hotspot: shape.hotspot,
                },
                (None, None) => Pointer::Hidden,
            };
            self.flips.drawn(output.id());
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
                output.fail(error);
            }
        }
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
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        let displays = self.0.borrow();
        if displays.outputs.is_empty() {
            return None;
        }
        Some(bounds(&displays.rects()).max.to_vec2() * displays.scale)
    }

    fn screens(&self) -> Option<Vec<Screen>> {
        let displays = self.0.borrow();
        Some(
            displays
                .outputs
                .iter()
                .map(|output| {
                    Screen::new(output.id(), output.name(), output.screen.rect)
                        .scaled(displays.scale)
                })
                .collect(),
        )
    }

    fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
        self.0.borrow_mut().prepare(output, scale, background)
    }

    fn present(&mut self, background: Color32) -> bool {
        self.0.borrow_mut().present(background);
        false
    }
}
