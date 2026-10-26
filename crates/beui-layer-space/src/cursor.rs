use beui_core::color::Color32;
use beui_core::context::{Context, WindowOutput};
use beui_core::damage::Region;
use beui_core::drawn_cursor;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::CursorIcon;
use beui_core::window::WindowId;
use beui_font_freetype::{FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT};

pub const COLOR: Color32 = Color32::WHITE;
pub const SIZE: Vec2 = Vec2::splat(drawn_cursor::EXTENT);
pub const HOTSPOT: Pos2 = Pos2::new(drawn_cursor::EXTENT / 2.0, drawn_cursor::EXTENT / 2.0);

pub struct Sprite {
    context: Context,
    pub window: WindowId,
    painted: Option<(CursorIcon, f32)>,
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
        }
    }

    pub fn repaint(&mut self) {
        self.painted = None;
    }

    pub fn changed(&self, icon: CursorIcon, scale: f32) -> bool {
        self.painted != Some((icon, scale))
    }

    pub fn paint(&mut self, icon: CursorIcon, scale: f32) -> WindowOutput {
        self.painted = Some((icon, scale));
        self.context.set_pixels_per_point(scale);
        let mut output = self.context.run(|context| {
            drawn_cursor::paint(&context.painter(), HOTSPOT, icon, COLOR);
        });
        let mut window = output.windows.swap_remove(0);
        window.id = self.window;
        window.size = SIZE;
        window.changed = true;
        window.damage = Region::from(Rect::from_min_size(Pos2::ZERO, SIZE));
        window.window.appearance.clear_color = Color32::TRANSPARENT;
        window
    }
}
