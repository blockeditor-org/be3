use crate::icons;
use crate::painter::Painter;
use crate::touch_cursor;
use beui_types::Color32;
use beui_types::CursorIcon;
use beui_types::FontId;
use beui_types::{Pos2, Rect, Vec2, pos2, vec2};

pub const SIZE: f32 = 26.0;
pub const EXTENT: f32 = SIZE * 3.0;
pub const SHADOW_OFFSET: f32 = 1.5;
pub const SHADOW: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 190);
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
            ibeam(painter, at + shadow, SHADOW);
            ibeam(painter, at, color);
        }
        icon => {
            let (text, hotspot, angle) = glyph(icon);
            let painter = painter.rotated(at, angle);
            let galley = painter.layout(text.as_str(), FontId::icons(SIZE), f32::INFINITY);
            let origin = at - hotspot * SIZE;
            painter.galley(origin + shadow, galley.clone(), SHADOW);
            painter.galley(origin, galley, color);
        }
    }
    bounds(at)
}

fn glyph(icon: CursorIcon) -> (icons::Glyph, Vec2, f32) {
    let center = CENTRE;
    let diagonal = std::f32::consts::FRAC_PI_4;
    match icon {
        CursorIcon::Default | CursorIcon::None | CursorIcon::Text => {
            (icons::ICON_MOUSE_POINTER_2, vec2(0.23, 0.19), 0.0)
        }
        CursorIcon::PointingHand => (icons::ICON_POINTER, vec2(0.29, 0.15), 0.0),
        CursorIcon::Crosshair | CursorIcon::Touch => (icons::ICON_PLUS, center, 0.0),
        CursorIcon::Grab => (icons::ICON_HAND, center, 0.0),
        CursorIcon::Grabbing => (icons::ICON_HAND, center, 0.0),
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
        CursorIcon::ResizeColumn | CursorIcon::ResizeEast | CursorIcon::ResizeWest => {
            (icons::ICON_MOVE_HORIZONTAL, center, 0.0)
        }
        CursorIcon::ResizeRow | CursorIcon::ResizeNorth | CursorIcon::ResizeSouth => {
            (icons::ICON_MOVE_VERTICAL, center, 0.0)
        }
        CursorIcon::ResizeNorthWest | CursorIcon::ResizeSouthEast => {
            (icons::ICON_MOVE_HORIZONTAL, center, diagonal)
        }
        CursorIcon::ResizeNorthEast | CursorIcon::ResizeSouthWest => {
            (icons::ICON_MOVE_HORIZONTAL, center, -diagonal)
        }
    }
}

fn ibeam(painter: &Painter, at: Pos2, color: Color32) {
    let half = IBEAM_HEIGHT / 2.0;
    let (top, bottom) = (at.y - half, at.y + half);
    painter.line(pos2(at.x, top), pos2(at.x, bottom), IBEAM_WIDTH, color);
    for y in [top, bottom] {
        painter.line(
            pos2(at.x - IBEAM_SERIF, y),
            pos2(at.x + IBEAM_SERIF, y),
            IBEAM_WIDTH,
            color,
        );
    }
}
