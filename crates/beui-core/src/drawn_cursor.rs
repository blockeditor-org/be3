use crate::font::Galley;
use crate::icons;
use crate::painter::Painter;
use crate::touch_cursor;
use beui_types::Color32;
use beui_types::CursorIcon;
use beui_types::FontId;
use beui_types::{Pos2, Rect, Vec2, pos2, vec2};

pub const SIZE: f32 = 26.0;
pub const EXTENT: f32 = SIZE * 3.0;
const SHADOW_OFFSET: f32 = 1.5;
const SHADOW: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 190);
const OUTLINE: f32 = 1.0;
const OUTLINE_COLOR: Color32 = Color32::from_gray(20);
pub const CENTRE: Vec2 = vec2(0.48, 0.56);
const IBEAM_HEIGHT: f32 = 18.0;
const IBEAM_SERIF: f32 = 3.5;
const IBEAM_WIDTH: f32 = 1.5;

pub fn bounds(at: Pos2) -> Rect {
    Rect::from_center_size(at, Vec2::splat(EXTENT))
}

pub fn paint(painter: &Painter, at: Pos2, icon: CursorIcon, color: Color32) -> Rect {
    let shadow = Vec2::splat(SHADOW_OFFSET);
    match icon {
        CursorIcon::None => {}
        CursorIcon::Touch => {
            touch_cursor::paint(painter, at);
        }
        CursorIcon::Text => {
            ibeam(painter, at + shadow, SHADOW, OUTLINE);
            ibeam(painter, at, OUTLINE_COLOR, OUTLINE);
            ibeam(painter, at, color, 0.0);
        }
        icon => {
            let (text, hotspot, angle) = glyph(icon);
            let painter = painter.rotated(at, angle);
            let galley = painter.layout(text.as_str(), FontId::icons(SIZE), f32::INFINITY);
            outlined(&painter, at - hotspot * SIZE, galley, color);
        }
    }
    bounds(at)
}

/// Draws an icon glyph as a cursor draws its own: `color` inside a dark outline, so it shows on
/// light and dark backgrounds alike, over a drop shadow.
pub fn outlined(painter: &Painter, origin: Pos2, galley: Galley, color: Color32) {
    painter.galley(origin + Vec2::splat(SHADOW_OFFSET), galley.clone(), SHADOW);
    let diagonal = OUTLINE * std::f32::consts::FRAC_1_SQRT_2;
    for offset in [
        vec2(OUTLINE, 0.0),
        vec2(-OUTLINE, 0.0),
        vec2(0.0, OUTLINE),
        vec2(0.0, -OUTLINE),
        vec2(diagonal, diagonal),
        vec2(-diagonal, diagonal),
        vec2(diagonal, -diagonal),
        vec2(-diagonal, -diagonal),
    ] {
        painter.galley(origin + offset, galley.clone(), OUTLINE_COLOR);
    }
    painter.galley(origin, galley, color);
}

fn glyph(icon: CursorIcon) -> (icons::Glyph, Vec2, f32) {
    let center = CENTRE;
    let diagonal = std::f32::consts::FRAC_PI_4;
    match icon {
        // `paint` draws these two without a glyph.
        CursorIcon::Default | CursorIcon::None | CursorIcon::Text => {
            (icons::ICON_MOUSE_POINTER_2, vec2(0.23, 0.19), 0.0)
        }
        CursorIcon::PointingHand => (icons::ICON_POINTER, vec2(0.29, 0.15), 0.0),
        CursorIcon::Crosshair | CursorIcon::Touch => (icons::ICON_PLUS, center, 0.0),
        CursorIcon::Grab => (icons::ICON_HAND, center, 0.0),
        CursorIcon::Grabbing => (icons::ICON_HAND_GRAB, center, 0.0),
        CursorIcon::NotAllowed => (icons::ICON_BAN, center, 0.0),
        CursorIcon::ResizeHorizontal => (icons::ICON_MOVE_HORIZONTAL, center, 0.0),
        CursorIcon::ResizeVertical => (icons::ICON_MOVE_VERTICAL, center, 0.0),
        CursorIcon::ResizeNwSe => (icons::ICON_MOVE_HORIZONTAL, center, diagonal),
        CursorIcon::ResizeNeSw => (icons::ICON_MOVE_HORIZONTAL, center, -diagonal),
        CursorIcon::Wait => (icons::ICON_HOURGLASS, center, 0.0),
        CursorIcon::Progress => (icons::ICON_LOADER_CIRCLE, center, 0.0),
        CursorIcon::Move => (icons::ICON_MOVE, center, 0.0),
        CursorIcon::Help => (icons::ICON_CIRCLE_QUESTION_MARK, center, 0.0),
        CursorIcon::Alias => (icons::ICON_CORNER_UP_RIGHT, center, 0.0),
        CursorIcon::ContextMenu => (icons::ICON_MENU, center, 0.0),
        CursorIcon::Cell => (icons::ICON_GRID_3X3, center, 0.0),
        CursorIcon::VerticalText => (icons::ICON_ARROW_DOWN_A_Z, center, 0.0),
        CursorIcon::Copy => (icons::ICON_COPY, center, 0.0),
        CursorIcon::NoDrop => (icons::ICON_CIRCLE_MINUS, center, 0.0),
        CursorIcon::AllScroll => (icons::ICON_MOVE, center, 0.0),
        CursorIcon::ZoomIn => (icons::ICON_ZOOM_IN, center, 0.0),
        CursorIcon::ZoomOut => (icons::ICON_ZOOM_OUT, center, 0.0),
        CursorIcon::ResizeColumn => (icons::ICON_MOVE_HORIZONTAL, center, 0.0),
        CursorIcon::ResizeRow => (icons::ICON_MOVE_VERTICAL, center, 0.0),
        CursorIcon::ResizeEast => (icons::ICON_ARROW_RIGHT_TO_LINE, center, 0.0),
        CursorIcon::ResizeWest => (icons::ICON_ARROW_LEFT_TO_LINE, center, 0.0),
        CursorIcon::ResizeNorth => (icons::ICON_ARROW_UP_TO_LINE, center, 0.0),
        CursorIcon::ResizeSouth => (icons::ICON_ARROW_DOWN_TO_LINE, center, 0.0),
        CursorIcon::ResizeNorthEast => (icons::ICON_ARROW_UP_RIGHT, center, 0.0),
        CursorIcon::ResizeNorthWest => (icons::ICON_ARROW_UP_LEFT, center, 0.0),
        CursorIcon::ResizeSouthEast => (icons::ICON_ARROW_DOWN_RIGHT, center, 0.0),
        CursorIcon::ResizeSouthWest => (icons::ICON_ARROW_DOWN_LEFT, center, 0.0),
    }
}

/// The text cursor, grown by `grow` on every side for its outline.
fn ibeam(painter: &Painter, at: Pos2, color: Color32, grow: f32) {
    let half = IBEAM_HEIGHT / 2.0;
    let (top, bottom) = (at.y - half, at.y + half);
    let width = IBEAM_WIDTH + 2.0 * grow;
    painter.line(
        pos2(at.x, top - grow),
        pos2(at.x, bottom + grow),
        width,
        color,
    );
    for y in [top, bottom] {
        painter.line(
            pos2(at.x - IBEAM_SERIF - grow, y),
            pos2(at.x + IBEAM_SERIF + grow, y),
            width,
            color,
        );
    }
}
