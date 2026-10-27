use block_editor_beui::beui::icons::Glyph;
use std::rc::Rc;

use block_editor_beui::beui::{
    Color32, FontId, Image, LineCap, LineJoin, Pos2, Rect, Stroke, TextLayout, Vec2, pos2,
    reactive::Painter,
};
use infinite_canvas_types::{
    CanvasArrowhead, CanvasColor, CanvasEntity, CanvasFill, CanvasLook, CanvasPoint, CanvasShape,
    CanvasStrokeStyle, CanvasStyle,
};

use crate::geometry::*;

use super::sketch::{Ink, Mark, Sketch};

pub(crate) const SELECTION: Color32 = Color32::from_rgb(140, 200, 255);
const PLACEHOLDER: Color32 = Color32::from_rgb(35, 35, 35);
const ARROW_SPREAD: f32 = 0.45;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    pub(crate) origin: Pos2,
    pub(crate) scale: f32,
}

impl Camera {
    pub(crate) fn item(corner: Pos2, scale: f32) -> Self {
        Self {
            origin: pos2(-corner.x * scale, -corner.y * scale),
            scale,
        }
    }

    pub(crate) fn at(self, at: CanvasPoint) -> Pos2 {
        pos2(
            self.origin.x + at.x * self.scale,
            self.origin.y + at.y * self.scale,
        )
    }

    pub(crate) fn rect(self, bounds: WorldRect) -> Rect {
        Rect::from_min_max(self.at(bounds.min), self.at(bounds.max))
    }
}

pub(crate) fn with_opacity(color: Color32, opacity: f32) -> Color32 {
    let [red, green, blue, alpha] = color.to_array();
    Color32::from_rgba_unmultiplied(
        red,
        green,
        blue,
        (alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(crate) fn resolve_color(color: CanvasColor, auto: Color32) -> Color32 {
    match color {
        CanvasColor::Auto => auto,
        CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        } => Color32::from_rgba_unmultiplied(red, green, blue, alpha),
        CanvasColor::Palette { hue, tone } => super::style::shade(hue, tone, auto),
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Palette {
    pub(crate) auto: Color32,
    pub(crate) surface: Color32,
    pub(crate) border: Color32,
    pub(crate) muted: Color32,
}

#[derive(Clone)]
pub(crate) struct EntityPaint {
    pub(crate) entity: CanvasEntity,
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) title: String,
    pub(crate) glyph: Option<Glyph>,
    pub(crate) automatic: bool,
    pub(crate) covered: bool,
    pub(crate) placeholder: Option<Image>,
    pub(crate) sketch: Rc<Sketch>,
}

impl PartialEq for EntityPaint {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
            && self.camera == other.camera
            && self.palette == other.palette
            && self.title == other.title
            && self.glyph == other.glyph
            && self.automatic == other.automatic
            && self.covered == other.covered
            && self.placeholder == other.placeholder
            && Rc::ptr_eq(&self.sketch, &other.sketch)
    }
}

impl EntityPaint {
    pub(crate) fn draw(&self, painter: &Painter) {
        if self.entity.hidden {
            return;
        }
        let style = self.entity.style;
        let opacity = style.opacity.clamp(0.0, 1.0);
        let color = with_opacity(
            resolve_color(style.stroke_color, self.palette.auto),
            opacity,
        );
        let width = (style.stroke_width.max(0.0) * self.camera.scale).max(0.1);
        let fill = with_opacity(resolve_color(style.fill_color, self.palette.auto), opacity);
        let clean = style.look == CanvasLook::Clean;
        match self.entity.shape {
            CanvasShape::Rectangle if clean => self.draw_rectangle(painter, color, fill, width),
            CanvasShape::Rectangle
            | CanvasShape::Ellipse
            | CanvasShape::Diamond
            | CanvasShape::Path
            | CanvasShape::Freehand => self.sketch.draw(painter, color, fill),
            CanvasShape::Text | CanvasShape::Frame => {}
            CanvasShape::Image => {
                let painter = self.turned(painter);
                self.fill_uncovered(&painter, self.box_rect(), opacity);
            }
            CanvasShape::Block => self.draw_block(painter, color, opacity),
            CanvasShape::Editor => self.draw_direct_editor(painter, color, opacity),
        }
    }

    fn at(&self, point: CanvasPoint) -> Pos2 {
        self.camera.at(point)
    }

    fn box_rect(&self) -> Rect {
        let center = self.at(self.entity.transform.center);
        let size = Vec2::new(
            self.entity.transform.size.x * self.camera.scale,
            self.entity.transform.size.y * self.camera.scale,
        );
        Rect::from_min_max(
            pos2(center.x - size.x / 2.0, center.y - size.y / 2.0),
            pos2(center.x + size.x / 2.0, center.y + size.y / 2.0),
        )
    }

    fn turned(&self, painter: &Painter) -> Painter {
        painter.rotated(
            self.at(self.entity.transform.center),
            self.entity.transform.rotation,
        )
    }

    fn draw_rectangle(&self, painter: &Painter, color: Color32, fill: Color32, width: f32) {
        let turned = self.turned(painter);
        let rect = self.box_rect();
        let radius = (self.entity.style.corner_radius * self.camera.scale).max(0.0);
        let solid = self.entity.style.stroke_style == CanvasStrokeStyle::Solid;
        if self.entity.style.fill == CanvasFill::Solid {
            turned.rect_filled(rect, radius, fill);
        }
        self.sketch
            .draw_only(painter, color, fill, |part| match part.mark {
                Mark::Fill(_) | Mark::Painted(..) => false,
                Mark::Stroke(..) => part.ink == Ink::Fill || !solid,
            });
        if solid {
            turned.rect_stroke(rect, radius, width, color);
        }
    }

    fn fill_uncovered(&self, painter: &Painter, rect: Rect, opacity: f32) {
        if self.covered {
            return;
        }
        match &self.placeholder {
            Some(placeholder) => painter.image(
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                placeholder,
                with_opacity(Color32::WHITE, opacity),
                0.0,
                true,
            ),
            None => painter.rect_filled(rect, 0.0, with_opacity(PLACEHOLDER, opacity)),
        }
    }

    fn draw_block(&self, painter: &Painter, color: Color32, opacity: f32) {
        let painter = self.turned(painter);
        let rect = self.box_rect();
        self.fill_uncovered(&painter, rect, opacity);
        let title_size = (18.0 * self.camera.scale).clamp(8.0, 42.0);
        let note_size = (12.0 * self.camera.scale).clamp(7.0, 30.0);
        let title = painter.lay_out_text(
            &self.title,
            FontId::proportional(title_size),
            TextLayout::wrapped(rect.width()),
        );
        let note = painter.lay_out_text(
            "(TODO: preview)",
            FontId::proportional(note_size),
            TextLayout::wrapped(rect.width()),
        );
        let gap = (4.0 * self.camera.scale).clamp(2.0, 10.0);
        let total = title.size().y + gap + note.size().y;
        let top = rect.center().y - total / 2.0;
        painter.draw_text(
            pos2(rect.center().x - title.size().x / 2.0, top),
            &title,
            color,
        );
        painter.draw_text(
            pos2(
                rect.center().x - note.size().x / 2.0,
                top + total - note.size().y,
            ),
            &note,
            color,
        );
    }

    fn draw_direct_editor(&self, painter: &Painter, color: Color32, opacity: f32) {
        let scale = self.entity.scale;
        let Some(layout) = direct_editor_layout(&self.entity) else {
            return;
        };
        let outer = self.camera.rect(entity_bounds(&self.entity));
        let title_bar = self.camera.rect(layout.title_bar);
        let content = self.camera.rect(layout.content);
        let radius = (6.0 * scale * self.camera.scale).clamp(2.0, 12.0);
        painter.rect_filled(outer, radius, with_opacity(self.palette.surface, opacity));
        painter.rect_stroke(
            outer,
            radius,
            self.camera.scale.max(0.5),
            with_opacity(self.palette.border, opacity),
        );
        painter.rect_filled(
            title_bar,
            (4.0 * scale * self.camera.scale).clamp(1.0, 8.0),
            with_opacity(self.palette.muted, opacity * 0.25),
        );
        self.fill_uncovered(painter, content, opacity);
        let font_size = (16.0 * scale * self.camera.scale).clamp(8.0, 32.0);
        let padding = (6.0 * scale * self.camera.scale).clamp(3.0, 12.0);
        let painter = painter.with_clip_rect(title_bar);
        let mut x = title_bar.left() + padding;
        if let Some(glyph) = self.glyph {
            let icon = painter.lay_out_text(
                glyph.as_str(),
                FontId::icons(font_size),
                TextLayout::DEFAULT,
            );
            painter.draw_text(
                pos2(x, title_bar.center().y - icon.size().y / 2.0),
                &icon,
                color,
            );
            x += font_size + padding;
        }
        let title = painter.lay_out_text(
            &self.title,
            FontId::proportional(font_size),
            TextLayout::wrapped((title_bar.right() - x).max(1.0)),
        );
        painter.draw_text(
            pos2(x, title_bar.center().y - title.size().y / 2.0),
            &title,
            color,
        );
    }
}

pub(crate) fn path_middle(line: &[CanvasPoint]) -> CanvasPoint {
    let lengths: Vec<f32> = line
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .collect();
    let mut left = lengths.iter().sum::<f32>() / 2.0;
    for (pair, length) in line.windows(2).zip(&lengths) {
        if left <= *length {
            let t = left / length.max(f32::EPSILON);
            return CanvasPoint::new(
                pair[0].x + (pair[1].x - pair[0].x) * t,
                pair[0].y + (pair[1].y - pair[0].y) * t,
            );
        }
        left -= length;
    }
    line.first().copied().unwrap_or_default()
}

pub(crate) fn path_stroke(style: CanvasStrokeStyle, width: f32) -> Stroke {
    let stroke = Stroke::new(width).join(LineJoin::Round).cap(LineCap::Round);
    match style {
        CanvasStrokeStyle::Solid => stroke,
        CanvasStrokeStyle::Dashed => stroke.dashed([width * 3.0, width * 3.0], 0.0),
        CanvasStrokeStyle::Dotted => stroke.dashed([width * 0.01, width * 2.5], 0.0),
    }
}

pub(crate) fn arrowhead_size(stroke_width: f32) -> f32 {
    (stroke_width * 4.0).max(8.0)
}

pub(crate) fn arrow_ends(style: &CanvasStyle, line: &[Pos2]) -> Vec<(Pos2, Vec2, f32)> {
    let size = arrowhead_size(style.stroke_width);
    [
        (style.start_arrowhead, line.first(), line.get(1)),
        (
            style.end_arrowhead,
            line.last(),
            line.len().checked_sub(2).map(|at| &line[at]),
        ),
    ]
    .into_iter()
    .filter_map(|(head, tip, before)| {
        let (CanvasArrowhead::Arrow, Some(tip), Some(before)) = (head, tip, before) else {
            return None;
        };
        let delta = *before - *tip;
        let length = delta.length();
        (length > f32::EPSILON).then(|| (*tip, delta / length, size.min(length)))
    })
    .collect()
}

pub(crate) fn arrow_wings(tip: Pos2, inward: Vec2, size: f32) -> [Pos2; 2] {
    let base = pos2(tip.x + inward.x * size, tip.y + inward.y * size);
    let across = Vec2::new(-inward.y, inward.x);
    let spread = size * ARROW_SPREAD;
    [
        pos2(base.x + across.x * spread, base.y + across.y * spread),
        pos2(base.x - across.x * spread, base.y - across.y * spread),
    ]
}

pub(crate) fn arrowhead(
    painter: &Painter,
    tip: Pos2,
    inward: Vec2,
    size: f32,
    width: f32,
    color: Color32,
) {
    let width = width.max(size * 0.25);
    for wing in arrow_wings(tip, inward, size) {
        painter.line(tip, wing, width, color);
    }
}

pub(crate) fn outline(painter: &Painter, corners: [Pos2; 4], width: f32, color: Color32) {
    for index in 0..corners.len() {
        painter.line(
            corners[index],
            corners[(index + 1) % corners.len()],
            width,
            color,
        );
    }
}

pub(crate) fn handle(painter: &Painter, at: Pos2, color: Color32) {
    let rect = Rect::from_min_max(
        pos2(at.x - HANDLE_RADIUS, at.y - HANDLE_RADIUS),
        pos2(at.x + HANDLE_RADIUS, at.y + HANDLE_RADIUS),
    );
    painter.rect_filled(rect, HANDLE_RADIUS, color);
}
