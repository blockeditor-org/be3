use beui_core::color::Color32;
use beui_core::context::Context;
use beui_core::display::Layer;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
use beui_core::painter::{Corners, Shape};

pub const MARGIN: f32 = 24.0;
pub const PADDING: f32 = 16.0;
pub const WIDEST: f32 = 440.0;
const RADIUS: f32 = 12.0;
const GAP: f32 = 6.0;
const TITLE_SIZE: f32 = 18.0;
const DETAIL_SIZE: f32 = 14.0;
const CARD: Color32 = Color32::from_rgba_unmultiplied(30, 34, 43, 235);
const TITLE: Color32 = Color32::from_rgb(236, 240, 246);
const DETAIL: Color32 = Color32::from_rgb(176, 184, 198);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Notice {
    pub title: String,
    pub detail: String,
}

pub fn card(size: Vec2, height: f32) -> Rect {
    let width = (size.x - 2.0 * MARGIN).clamp(0.0, WIDEST);
    Rect::from_min_size(pos2((size.x - width) / 2.0, MARGIN), vec2(width, height))
}

pub fn layers(notice: &Notice, context: &Context, size: Vec2, pixels_per_point: f32) -> Vec<Layer> {
    let wrap = (card(size, 0.0).width() - 2.0 * PADDING).max(1.0);
    let raster = pixels_per_point / context.pixels_per_point();
    let lay = |text: &str, font: FontId| {
        let galley = context.layout(text, font, TextLayout::wrapped(wrap));
        context.rastered(&galley, raster)
    };
    let title = lay(&notice.title, FontId::proportional(TITLE_SIZE).bold(true));
    let detail = lay(&notice.detail, FontId::proportional(DETAIL_SIZE));
    let height = 2.0 * PADDING + title.size().y + GAP + detail.size().y;
    let card = card(size, height);
    let inside = card.min + vec2(PADDING, PADDING);
    let below = inside + vec2(0.0, title.size().y + GAP);
    vec![
        Layer::Shape(Shape::Rect {
            rect: card,
            corner_radius: Corners::all(RADIUS),
            stroke_width: 0.0,
            color: CARD,
            rotation: Rotation::NONE,
            clip: Rect::EVERYTHING,
        }),
        text(inside, title, TITLE),
        text(below, detail, DETAIL),
    ]
}

fn text(origin: Pos2, galley: Galley, color: Color32) -> Layer {
    Layer::Shape(Shape::Text {
        origin,
        galley,
        scale: 1.0,
        color,
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    })
}
