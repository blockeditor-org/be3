use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui::{
    Color32, CursorIcon, GpuSetup, Pos2, Rect, RendererInfo, Repainting, Setup, Vec2, WindowId,
    WindowOutput,
};
use beui_core::renderer::Renderer;

use crate::cursors::{Cursors, Shape};
use crate::flips::Flips;
use crate::gpu::{Gpu, SoftwareCursor};
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

pub struct Outputs {
    pub outputs: Vec<Output>,
    pub pointer: Pos2,
    pub icon: CursorIcon,
    pub gpu: Rc<Gpu>,
    pub flips: Flips,
    cursor: SoftwareCursor,
    cursors: Cursors,
    cursor_scale: f32,
    look: Look,
    placed: Option<Rect>,
    shown: HashMap<String, Option<WindowId>>,
}

impl Outputs {
    pub fn new(gpu: Rc<Gpu>, cursor: SoftwareCursor, scale: f32) -> Self {
        Self {
            outputs: Vec::new(),
            flips: Flips::default(),
            pointer: Pos2::ZERO,
            icon: CursorIcon::Default,
            gpu,
            cursor,
            cursors: Cursors::new(Theme::from_env(), scale),
            cursor_scale: scale,
            look: Look::Icon(CursorIcon::Default),
            placed: None,
            shown: HashMap::new(),
        }
    }

    pub fn under(&self, position: Pos2) -> Option<&Output> {
        self.outputs
            .iter()
            .find(|output| output.screen.rect.contains(position))
    }

    pub fn follow_scales(&mut self) {
        let scale = self
            .outputs
            .iter()
            .map(|output| output.screen.scale)
            .fold(0.0_f32, f32::max);
        if scale > 0.0 && scale != self.cursor_scale {
            self.cursor_scale = scale;
            self.cursors = Cursors::new(Theme::from_env(), scale);
            self.replace_pointer();
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

    pub fn prepare<'a>(&mut self, window_of: impl Fn(&str) -> Option<&'a WindowOutput>) -> bool {
        if self.look() != self.look {
            self.replace_pointer();
        }
        for output in &mut self.outputs {
            let window = window_of(output.id());
            let shown = window.map(|window| window.id);
            let empty;
            let window = match window {
                Some(window) => window,
                None => {
                    empty = blank(output.screen.size());
                    &empty
                }
            };
            let background = window.window.appearance.clear_color;
            output.clear = background;
            if self.shown.insert(output.id().to_owned(), shown) != Some(shown) {
                output.screen.invalidate();
            } else if window.changed {
                output.screen.damage(window.repaint(background));
            }
            output.screen.prepare(&self.gpu, window);
        }
        let connected: Vec<String> = self
            .outputs
            .iter()
            .map(|output| output.id().to_owned())
            .collect();
        self.shown.retain(|id, _| connected.contains(id));
        self.outputs.iter().any(Output::wants_frame)
    }

    fn present(&mut self) {
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
            let clear = output.clear;
            let drawn = output.render(
                &gpu,
                Frame {
                    clear,
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

fn blank(size: Vec2) -> WindowOutput {
    WindowOutput {
        id: WindowId::MAIN,
        size,
        layers: Rc::new(Vec::new()),
        window: beui_core::window::WindowState::default(),
        changed: true,
        damage: beui::Region::NOTHING,
        moved: None,
        pixels_per_point: 1.0,
    }
}

pub struct DisplayRenderer(pub Rc<RefCell<Outputs>>);

impl Renderer for DisplayRenderer {
    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![("Surface format", format!("{FORMAT:?}"))],
        }
    }

    fn provide(&self, setup: &mut Setup) {
        let outputs = self.0.borrow();
        setup.provide(GpuSetup {
            device: outputs.gpu.device().clone(),
            queue: outputs.gpu.queue().clone(),
            format: FORMAT,
        });
        setup.provide(outputs.cursor.clone());
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        let outputs = self.0.borrow();
        let first = outputs.outputs.first()?;
        let (width, height) = first.screen.picture();
        Some(Vec2::new(width as f32, height as f32))
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        false
    }

    fn note(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) {}

    fn present(&mut self, _background: Color32) -> bool {
        self.0.borrow_mut().present();
        false
    }
}
