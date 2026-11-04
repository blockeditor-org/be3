use std::f32::consts::TAU;

use beui_core::context::{Context, WindowOutput};
use beui_core::drawn_cursor;
use beui_core::icons;
use beui_core::painter::Painter;
use beui_font_freetype::{FontLibrary, FontSources, FreetypeFonts};
use beui_types::Color32;
use beui_types::CursorIcon;
use beui_types::FontId;
use beui_types::TextLayout;
use beui_types::WindowId;
use beui_types::geometry::Region;
use beui_types::{Pos2, Rect, Vec2, pos2, vec2};

pub const COLOR: Color32 = Color32::WHITE;
pub const SIZE: Vec2 = Vec2::splat(drawn_cursor::EXTENT);
pub const HOTSPOT: Pos2 = Pos2::new(drawn_cursor::EXTENT / 2.0, drawn_cursor::EXTENT / 2.0);
pub const AWAY_INSET: f32 = 36.0;
pub const AWAY_STEPS: i32 = 90;
const MOUSE_SIZE: f32 = 22.0;
const ARROW_SIZE: f32 = 18.0;
const ARROW_OFFSET: f32 = 22.0;
const LABEL_SIZE: f32 = 16.0;
const LABEL_PADDING: f32 = 8.0;
const LABEL_RADIUS: f32 = 6.0;
const LABEL_CARD: Color32 = Color32::from_rgba_unmultiplied(30, 34, 43, 235);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Icon(CursorIcon),
    Away(i32),
}

impl Look {
    pub fn away(angle: f32) -> Self {
        let steps = (angle / TAU * AWAY_STEPS as f32).round() as i32;
        Self::Away(steps.rem_euclid(AWAY_STEPS))
    }

    pub fn angle(steps: i32) -> f32 {
        steps as f32 / AWAY_STEPS as f32 * TAU
    }
}

pub struct Sprite {
    context: Context,
    pub window: WindowId,
    painted: Option<(Look, Option<String>, f32)>,
    output: Option<WindowOutput>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self::new()
    }
}

impl Sprite {
    pub fn new() -> Self {
        Self {
            context: Context::new(FreetypeFonts::new(FontLibrary::new(FontSources::bundled()))),
            window: WindowId::next(),
            painted: None,
            output: None,
        }
    }

    pub fn repaint(&mut self) {
        self.painted = None;
    }

    pub fn changed(&self, look: Look, label: Option<&str>, scale: f32) -> bool {
        self.painted.as_ref().is_none_or(|(painted, text, at)| {
            (*painted, text.as_deref(), *at) != (look, label, scale)
        })
    }

    pub fn size(&self) -> Vec2 {
        self.output.as_ref().map_or(SIZE, |output| output.size)
    }

    pub fn output(&self) -> Option<&WindowOutput> {
        self.output.as_ref()
    }

    pub fn paint(&mut self, look: Look, label: Option<&str>, scale: f32) -> WindowOutput {
        self.painted = Some((look, label.map(str::to_owned), scale));
        self.context.set_pixels_per_point(scale);
        let galley = label.map(|label| {
            self.context
                .layout(label, FontId::proportional(LABEL_SIZE), TextLayout::DEFAULT)
        });
        let card = galley.as_ref().map(|galley| {
            Rect::from_min_size(
                pos2(SIZE.x, HOTSPOT.y),
                galley.size() + Vec2::splat(2.0 * LABEL_PADDING),
            )
        });
        let size = card.map_or(SIZE, |card| vec2(card.max.x, card.max.y.max(SIZE.y)));
        let mut output = self.context.run(|context| {
            let painter = context.painter();
            match look {
                Look::Icon(icon) => {
                    drawn_cursor::paint(&painter, HOTSPOT, icon, COLOR);
                }
                Look::Away(steps) => away(&painter, Look::angle(steps)),
            }
            if let (Some(card), Some(galley)) = (card, galley.clone()) {
                painter.rect_filled(card, LABEL_RADIUS, LABEL_CARD);
                painter.galley(card.min + Vec2::splat(LABEL_PADDING), galley, COLOR);
            }
        });
        let mut window = output.windows.swap_remove(0);
        window.id = self.window;
        window.size = size;
        window.changed = true;
        window.damage = Region::from(Rect::from_min_size(Pos2::ZERO, size));
        window.window.appearance.clear_color = Color32::TRANSPARENT;
        self.output = Some(window.clone());
        window
    }
}

fn away(painter: &Painter, angle: f32) {
    glyph(painter, icons::ICON_MOUSE, HOTSPOT, MOUSE_SIZE);
    let arrow = painter.rotated(HOTSPOT, angle);
    glyph(
        &arrow,
        icons::ICON_ARROW_UP,
        HOTSPOT - vec2(0.0, ARROW_OFFSET),
        ARROW_SIZE,
    );
}

fn glyph(painter: &Painter, glyph: icons::Glyph, centre: Pos2, size: f32) {
    let galley = painter.layout(glyph.as_str(), FontId::icons(size), f32::INFINITY);
    drawn_cursor::outlined(painter, centre - drawn_cursor::CENTRE * size, galley, COLOR);
}
