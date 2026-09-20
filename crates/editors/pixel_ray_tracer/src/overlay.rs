use std::rc::Rc;

use block_client::blocks::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_PALETTE, PIXEL_RAY_TRACER_SIZE, Point, RayEntity,
};
use block_editor_plugin::beui::reactive::Draw;
use block_editor_plugin::beui::{Color32, Painter, Pos2, Rect, Vec2, pos2};

const SURFACE_WIDTH: f32 = 1.5;
const WATER_FILL: Color32 = Color32::from_rgba_unmultiplied(41, 173, 255, 60);
const WATER_EDGE: Color32 = Color32::from_rgb(100, 220, 255);
const WATER_EDGE_WIDTH: f32 = 0.5;
const LIGHT_RADIUS: f32 = 2.5;
const LIGHT_EDGE_WIDTH: f32 = 0.4;
const HANDLE_RADIUS: f32 = 1.6;
const SELECTION: Color32 = Color32::from_rgb(255, 255, 0);
const SELECTION_GAP: f32 = 1.2;
const SELECTION_WIDTH: f32 = 0.6;
const PREVIEW_WIDTH: f32 = 2.0;

pub(crate) fn palette_color(index: u8) -> Color32 {
    let rgb = PIXEL_RAY_TRACER_PALETTE[usize::from(index)];
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

#[derive(Clone, PartialEq)]
pub(crate) enum Preview {
    None,
    Pixels(Vec<(u16, u16)>, u8),
    Surface(Point, Point, u8),
    Water(Point, Point),
}

struct Artwork {
    origin: Pos2,
    scale: f32,
}

impl Artwork {
    fn new(rect: Rect) -> Self {
        Self {
            origin: rect.min,
            scale: rect.width() / f32::from(PIXEL_RAY_TRACER_SIZE),
        }
    }

    fn at(&self, point: Point) -> Pos2 {
        pos2(
            self.origin.x + point.x * self.scale,
            self.origin.y + point.y * self.scale,
        )
    }

    fn length(&self, world: f32) -> f32 {
        world * self.scale
    }

    fn square(&self, at: Point, radius: f32) -> Rect {
        let radius = self.length(radius);
        let center = self.at(at);
        Rect::from_min_size(
            pos2(center.x - radius, center.y - radius),
            Vec2::splat(radius * 2.0),
        )
    }

    fn span(&self, start: Point, end: Point) -> Rect {
        Rect::from_min_max(
            self.at(Point::new(start.x.min(end.x), start.y.min(end.y))),
            self.at(Point::new(start.x.max(end.x), start.y.max(end.y))),
        )
    }

    fn segment(&self, painter: &Painter, start: Point, end: Point, width: f32, color: Color32) {
        painter.line(self.at(start), self.at(end), self.length(width), color);
    }

    fn disc(&self, painter: &Painter, at: Point, radius: f32, color: Color32) {
        let square = self.square(at, radius);
        painter.rect_filled(square, square.width() / 2.0, color);
    }

    fn ring(&self, painter: &Painter, at: Point, radius: f32, width: f32, color: Color32) {
        let square = self.square(at, radius);
        painter.rect_stroke(
            square,
            square.width() / 2.0,
            self.length(width).max(1.0),
            color,
        );
    }
}

pub(crate) fn draw(entities: Vec<RayEntity>, selected: Option<u64>, preview: Preview) -> Draw {
    Rc::new(move |painter: &Painter, rect: Rect| {
        let artwork = Artwork::new(rect);
        for entity in &entities {
            paint_entity(painter, &artwork, entity);
            if selected == Some(entity.id()) {
                paint_selection(painter, &artwork, entity);
            }
        }
        paint_preview(painter, &artwork, &preview);
    })
}

fn paint_entity(painter: &Painter, artwork: &Artwork, entity: &RayEntity) {
    match entity {
        RayEntity::Surface {
            start,
            end,
            color_index,
            ..
        } => artwork.segment(
            painter,
            *start,
            *end,
            SURFACE_WIDTH,
            palette_color(*color_index),
        ),
        RayEntity::Water { start, end, .. } => {
            painter.rect_filled(artwork.span(*start, *end), 0.0, WATER_FILL);
            painter.rect_stroke(
                artwork.span(*start, *end),
                0.0,
                artwork.length(WATER_EDGE_WIDTH).max(1.0),
                WATER_EDGE,
            );
        }
        RayEntity::Light {
            position,
            color_index,
            ..
        } => {
            artwork.disc(
                painter,
                *position,
                LIGHT_RADIUS,
                palette_color(*color_index),
            );
            artwork.ring(
                painter,
                *position,
                LIGHT_RADIUS,
                LIGHT_EDGE_WIDTH,
                Color32::WHITE,
            );
        }
    }
}

fn paint_selection(painter: &Painter, artwork: &Artwork, entity: &RayEntity) {
    match entity {
        RayEntity::Light { position, .. } => artwork.ring(
            painter,
            *position,
            LIGHT_RADIUS + SELECTION_GAP,
            SELECTION_WIDTH,
            SELECTION,
        ),
        RayEntity::Surface { start, end, .. } | RayEntity::Water { start, end, .. } => {
            artwork.disc(painter, *start, HANDLE_RADIUS, Color32::WHITE);
            artwork.disc(painter, *end, HANDLE_RADIUS, Color32::WHITE);
        }
    }
}

fn paint_preview(painter: &Painter, artwork: &Artwork, preview: &Preview) {
    match preview {
        Preview::None => {}
        Preview::Pixels(points, color_index) => {
            let color = palette_color(*color_index);
            for point in points {
                let start = Point::new(f32::from(point.0), f32::from(point.1));
                let cell = artwork.span(start, Point::new(start.x + 1.0, start.y + 1.0));
                painter.rect_filled(cell, 0.0, color);
            }
        }
        Preview::Surface(start, end, color_index) => artwork.segment(
            painter,
            *start,
            *end,
            PREVIEW_WIDTH,
            palette_color(*color_index),
        ),
        Preview::Water(start, end) => {
            painter.rect_filled(artwork.span(*start, *end), 0.0, WATER_FILL);
        }
    }
}

#[cfg(test)]
mod tests;
