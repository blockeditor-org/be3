use block_client::blocks::infinite_canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTextAlign,
    CanvasTextStyle, CanvasTextWeight,
};
use block_editor_plugin::beui::reactive::CanvasView;
use block_editor_plugin::beui::{
    Color32, FontId, Painter, Pos2, Rect, TextAlign, TextLayout, Vec2, pos2,
};

use crate::geometry::*;

pub(crate) const SELECTION: Color32 = Color32::from_rgb(140, 200, 255);
pub(crate) const PREVIEW_REGION: Color32 = Color32::from_rgb(245, 180, 60);
const PLACEHOLDER: Color32 = Color32::from_rgb(35, 35, 35);
const ARROW_SPREAD: f32 = 0.45;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    pub(crate) origin: Pos2,
    pub(crate) scale: f32,
}

impl Camera {
    pub(crate) fn of(view: Option<CanvasView>, fallback: Pos2) -> Self {
        let view = view.unwrap_or_else(|| CanvasView::new(fallback, 1.0));
        Self {
            origin: view.origin,
            scale: view.scale.max(f32::EPSILON),
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
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Palette {
    pub(crate) auto: Color32,
    pub(crate) surface: Color32,
    pub(crate) border: Color32,
    pub(crate) muted: Color32,
}

pub(crate) type TextMeasure = std::rc::Rc<std::cell::Cell<Option<(uuid::Uuid, Vec2)>>>;

#[derive(Clone)]
pub(crate) struct EntityPaint {
    pub(crate) entity: CanvasEntity,
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) title: String,
    pub(crate) glyph: Option<String>,
    pub(crate) automatic: bool,
    pub(crate) covered: bool,
    pub(crate) measure: Option<TextMeasure>,
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
            && match (&self.measure, &other.measure) {
                (Some(ours), Some(theirs)) => std::rc::Rc::ptr_eq(ours, theirs),
                (None, None) => true,
                _ => false,
            }
    }
}

impl EntityPaint {
    pub(crate) fn draw(&self, painter: &Painter) {
        let opacity = self.entity.style.opacity.clamp(0.0, 1.0);
        let color = with_opacity(
            resolve_color(self.entity.style.foreground, self.palette.auto),
            opacity,
        );
        let width = (self.entity.style.line_width.max(0.0) * self.camera.scale).max(0.1);
        match &self.entity.kind {
            CanvasEntityKind::Line => self.draw_line(painter, color, width),
            CanvasEntityKind::Rectangle => self.draw_rectangle(painter, color, width, opacity),
            CanvasEntityKind::Text {
                text,
                text_style,
                placeholder,
            } => self.draw_text(painter, color, text, *text_style, placeholder),
            CanvasEntityKind::Pen { points } => self.draw_pen(painter, color, width, points),
            CanvasEntityKind::Block { .. } => self.draw_block(painter, color, opacity),
            CanvasEntityKind::DirectEditor { scale, .. } => {
                self.draw_direct_editor(painter, color, opacity, *scale)
            }
        }
    }

    fn at(&self, point: CanvasPoint) -> Pos2 {
        self.camera.at(point)
    }

    fn local(&self, x: f32, y: f32) -> Pos2 {
        self.at(local_to_world(
            self.entity.transform,
            CanvasPoint::new(x, y),
        ))
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

    fn draw_line(&self, painter: &Painter, color: Color32, width: f32) {
        styled_line(
            painter,
            self.local(-0.5, 0.0),
            self.local(0.5, 0.0),
            width,
            color,
            self.entity.style,
            self.camera.scale,
        );
    }

    fn draw_rectangle(&self, painter: &Painter, color: Color32, width: f32, opacity: f32) {
        let painter = self.turned(painter);
        let rect = self.box_rect();
        let radius = (self.entity.style.corner_radius * self.camera.scale).max(0.0);
        if let Some(fill) = self.entity.style.fill {
            let fill = with_opacity(resolve_color(fill, self.palette.auto), opacity);
            painter.rect_filled(rect, radius, fill);
        }
        painter.rect_stroke(rect, radius, width, color);
    }

    fn draw_pen(&self, painter: &Painter, color: Color32, width: f32, points: &[CanvasPoint]) {
        for window in points.windows(2) {
            painter.line(
                self.at(local_to_world(self.entity.transform, window[0])),
                self.at(local_to_world(self.entity.transform, window[1])),
                width,
                color,
            );
        }
    }

    fn draw_text(
        &self,
        painter: &Painter,
        color: Color32,
        text: &str,
        text_style: CanvasTextStyle,
        placeholder: &str,
    ) {
        let (shown, color) = match text.is_empty() {
            true => (placeholder, with_opacity(color, 0.4)),
            false => (text, color),
        };
        let painter = self.turned(painter);
        let rect = self.box_rect();
        let font_size = (text_style.font_size * self.camera.scale).clamp(4.0, 256.0);
        let layout = TextLayout {
            wrap_width: match text_style.wrap {
                true => rect.width().max(1.0),
                false => f32::INFINITY,
            },
            align: match text_style.alignment {
                CanvasTextAlign::Left => TextAlign::Start,
                CanvasTextAlign::Center => TextAlign::Center,
                CanvasTextAlign::Right => TextAlign::End,
            },
            line_spacing: text_style.line_height.max(0.5),
            ..TextLayout::DEFAULT
        };
        let galley = painter.layout_text(shown, FontId::proportional(font_size), layout);
        let size = galley.size();
        if let Some(measure) = &self.measure {
            measure.set(Some((
                self.entity.id,
                Vec2::new(size.x / self.camera.scale, size.y / self.camera.scale),
            )));
        }
        let x = match text_style.alignment {
            CanvasTextAlign::Left => rect.left(),
            CanvasTextAlign::Center => rect.center().x - size.x / 2.0,
            CanvasTextAlign::Right => rect.right() - size.x,
        };
        let origin = pos2(x, rect.center().y - size.y / 2.0);
        if text_style.weight == CanvasTextWeight::Bold {
            painter.galley(
                pos2(origin.x + 0.6 * self.camera.scale, origin.y),
                galley.clone(),
                color,
            );
        }
        painter.galley(origin, galley, color);
    }

    fn draw_block(&self, painter: &Painter, color: Color32, opacity: f32) {
        let painter = self.turned(painter);
        let rect = self.box_rect();
        if !self.covered {
            painter.rect_filled(rect, 0.0, with_opacity(PLACEHOLDER, opacity));
        }
        let title_size = (18.0 * self.camera.scale).clamp(8.0, 42.0);
        let note_size = (12.0 * self.camera.scale).clamp(7.0, 30.0);
        let title = painter.layout(
            self.title.clone(),
            FontId::proportional(title_size),
            rect.width(),
        );
        let note = painter.layout(
            "(TODO: preview)",
            FontId::proportional(note_size),
            rect.width(),
        );
        let gap = (4.0 * self.camera.scale).clamp(2.0, 10.0);
        let total = title.size().y + gap + note.size().y;
        let top = rect.center().y - total / 2.0;
        painter.galley(
            pos2(rect.center().x - title.size().x / 2.0, top),
            title,
            color,
        );
        painter.galley(
            pos2(
                rect.center().x - note.size().x / 2.0,
                top + total - note.size().y,
            ),
            note,
            color,
        );
    }

    fn draw_direct_editor(&self, painter: &Painter, color: Color32, opacity: f32, scale: f32) {
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
        if !self.covered {
            painter.rect_filled(content, 0.0, with_opacity(PLACEHOLDER, opacity));
        }
        let font_size = (16.0 * scale * self.camera.scale).clamp(8.0, 32.0);
        let padding = (6.0 * scale * self.camera.scale).clamp(3.0, 12.0);
        let painter = painter.with_clip_rect(title_bar);
        let mut x = title_bar.left() + padding;
        if let Some(glyph) = &self.glyph {
            let icon = painter.layout(glyph.clone(), FontId::icons(font_size), f32::INFINITY);
            painter.galley(
                pos2(x, title_bar.center().y - icon.size().y / 2.0),
                icon,
                color,
            );
            x += font_size + padding;
        }
        let title = painter.layout(
            self.title.clone(),
            FontId::proportional(font_size),
            (title_bar.right() - x).max(1.0),
        );
        painter.galley(
            pos2(x, title_bar.center().y - title.size().y / 2.0),
            title,
            color,
        );
    }
}

pub(crate) fn styled_line(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    width: f32,
    color: Color32,
    style: CanvasEntityStyle,
    zoom: f32,
) {
    let delta = Vec2::new(end.x - start.x, end.y - start.y);
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = Vec2::new(delta.x / length, delta.y / length);
    let arrow = ((style.line_width * 4.0).max(8.0) * zoom).min(length * 0.4);
    let inset = arrow * 0.75;
    let along = |from: Pos2, amount: f32| {
        pos2(from.x + direction.x * amount, from.y + direction.y * amount)
    };
    let shaft_start = match style.arrow_start {
        true => along(start, inset),
        false => start,
    };
    let shaft_end = match style.arrow_end {
        true => along(end, -inset),
        false => end,
    };
    match style.dashed {
        true => dashed_line(painter, shaft_start, shaft_end, width, color),
        false => painter.line(shaft_start, shaft_end, width, color),
    }
    if style.arrow_start {
        arrowhead(painter, start, direction, arrow, width, color);
    }
    if style.arrow_end {
        arrowhead(
            painter,
            end,
            Vec2::new(-direction.x, -direction.y),
            arrow,
            width,
            color,
        );
    }
}

fn dashed_line(painter: &Painter, start: Pos2, end: Pos2, width: f32, color: Color32) {
    let delta = Vec2::new(end.x - start.x, end.y - start.y);
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = Vec2::new(delta.x / length, delta.y / length);
    let dash = (width * 3.0).max(2.0);
    let gap = (width * 2.0).max(2.0);
    let mut travelled = 0.0;
    while travelled < length {
        let to = (travelled + dash).min(length);
        painter.line(
            pos2(
                start.x + direction.x * travelled,
                start.y + direction.y * travelled,
            ),
            pos2(start.x + direction.x * to, start.y + direction.y * to),
            width,
            color,
        );
        travelled = to + gap;
    }
}

pub(crate) fn arrowhead(
    painter: &Painter,
    tip: Pos2,
    inward: Vec2,
    size: f32,
    width: f32,
    color: Color32,
) {
    let base = pos2(tip.x + inward.x * size, tip.y + inward.y * size);
    let across = Vec2::new(-inward.y, inward.x);
    let spread = size * ARROW_SPREAD;
    let width = width.max(size * 0.25);
    painter.line(
        tip,
        pos2(base.x + across.x * spread, base.y + across.y * spread),
        width,
        color,
    );
    painter.line(
        tip,
        pos2(base.x - across.x * spread, base.y - across.y * spread),
        width,
        color,
    );
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
