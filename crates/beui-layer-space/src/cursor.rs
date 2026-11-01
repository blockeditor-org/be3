use std::f32::consts::TAU;

use beui_core::context::{Context, WindowOutput};
use beui_core::drawn_cursor;
use beui_core::icons;
use beui_core::painter::Painter;
use beui_font_freetype::{FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT};
use beui_types::Color32;
use beui_types::CursorIcon;
use beui_types::FontId;
use beui_types::WindowId;
use beui_types::geometry::Region;
use beui_types::{Pos2, Rect, Vec2, vec2};

pub const COLOR: Color32 = Color32::WHITE;
pub const SIZE: Vec2 = Vec2::splat(drawn_cursor::EXTENT);
pub const HOTSPOT: Pos2 = Pos2::new(drawn_cursor::EXTENT / 2.0, drawn_cursor::EXTENT / 2.0);
pub const AWAY_INSET: f32 = 36.0;
pub const AWAY_STEPS: i32 = 90;
const MOUSE_SIZE: f32 = 22.0;
const ARROW_SIZE: f32 = 18.0;
const ARROW_OFFSET: f32 = 22.0;

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
    painted: Option<(Look, f32)>,
    output: Option<WindowOutput>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self::new()
    }
}

impl Sprite {
    pub fn new() -> Self {
        let sources = FontSources {
            icons: vec![FontData::from_static(ICONS_FONT)],
            ..FontSources::default()
        };
        Self {
            context: Context::new(FreetypeFonts::new(FontLibrary::new(sources))),
            window: WindowId::next(),
            painted: None,
            output: None,
        }
    }

    pub fn repaint(&mut self) {
        self.painted = None;
    }

    pub fn changed(&self, look: Look, scale: f32) -> bool {
        self.painted != Some((look, scale))
    }

    pub fn output(&self) -> Option<&WindowOutput> {
        self.output.as_ref()
    }

    pub fn paint(&mut self, look: Look, scale: f32) -> WindowOutput {
        self.painted = Some((look, scale));
        self.context.set_pixels_per_point(scale);
        let mut output = self.context.run(|context| {
            let painter = context.painter();
            match look {
                Look::Icon(icon) => {
                    drawn_cursor::paint(&painter, HOTSPOT, icon, COLOR);
                }
                Look::Away(steps) => away(&painter, Look::angle(steps)),
            }
        });
        let mut window = output.windows.swap_remove(0);
        window.id = self.window;
        window.size = SIZE;
        window.changed = true;
        window.damage = Region::from(Rect::from_min_size(Pos2::ZERO, SIZE));
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
    let origin = centre - drawn_cursor::CENTRE * size;
    let shadow = Vec2::splat(drawn_cursor::SHADOW_OFFSET);
    painter.galley(origin + shadow, galley.clone(), drawn_cursor::SHADOW);
    painter.galley(origin, galley, COLOR);
}
