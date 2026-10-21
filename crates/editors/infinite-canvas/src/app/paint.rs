use block_editor_beui::be_block::canvas::{
    CanvasArrowhead, CanvasColor, CanvasEntity, CanvasPoint, CanvasShape, CanvasStrokeStyle,
    CanvasStyle,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{
    Color32, FontId, Image, Painter, Pos2, Rect, TextAlign, TextLayout, Vec2, pos2,
};

use crate::geometry::*;

pub(crate) const SELECTION: Color32 = Color32::from_rgb(140, 200, 255);
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

    pub(crate) fn item(corner: Pos2) -> Self {
        Self {
            origin: pos2(-corner.x, -corner.y),
            scale: 1.0,
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

#[derive(Clone)]
pub(crate) struct EntityPaint {
    pub(crate) entity: CanvasEntity,
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) title: String,
    pub(crate) glyph: Option<String>,
    pub(crate) automatic: bool,
    pub(crate) covered: bool,
    pub(crate) placeholder: Option<Image>,
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
    }
}

impl EntityPaint {
    pub(crate) fn draw(&self, painter: &Painter) {
        if self.entity.hidden {
            return;
        }
        let style = self.entity.style;
        let opacity = style.opacity.clamp(0.0, 1.0);
        let color = with_opacity(resolve_color(style.stroke_color, self.palette.auto), opacity);
        let width = (style.stroke_width.max(0.0) * self.camera.scale).max(0.1);
        match self.entity.shape {
            CanvasShape::Rectangle => self.draw_rectangle(painter, color, width, opacity),
            CanvasShape::Ellipse | CanvasShape::Diamond => {
                self.draw_outline(painter, color, width, opacity)
            }
            CanvasShape::Path => self.draw_path(painter, color, width),
            CanvasShape::Freehand => self.draw_pen(painter, color, width),
            CanvasShape::Text | CanvasShape::Frame => {}
            CanvasShape::Image => {
                let painter = self.turned(painter);
                self.fill_uncovered(&painter, self.box_rect(), opacity);
            }
            CanvasShape::Block => self.draw_block(painter, color, opacity),
            CanvasShape::Editor => self.draw_direct_editor(painter, color, opacity),
        }
        self.draw_label(painter, color);
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

    fn draw_path(&self, painter: &Painter, color: Color32, width: f32) {
        let line: Vec<Pos2> = flattened(&self.entity.world_points())
            .into_iter()
            .map(|point| self.at(point))
            .collect();
        styled_polyline(
            painter,
            &line,
            width,
            color,
            self.entity.style,
            self.camera.scale,
        );
    }

    fn draw_outline(&self, painter: &Painter, color: Color32, width: f32, opacity: f32) {
        let outline: Vec<Pos2> = outline_points(&self.entity)
            .into_iter()
            .map(|point| self.at(point))
            .collect();
        if let Some(fill) = self.entity.style.filled() {
            fill_polygon(
                painter,
                &outline,
                with_opacity(resolve_color(fill, self.palette.auto), opacity),
            );
        }
        let mut closed = outline.clone();
        closed.extend(outline.first().copied());
        stroke_polyline(painter, &closed, width, color, self.entity.style.stroke_style);
    }

    fn draw_label(&self, painter: &Painter, color: Color32) {
        let entity = &self.entity;
        if entity.text.is_empty()
            || matches!(
                entity.shape,
                CanvasShape::Text | CanvasShape::Frame | CanvasShape::Editor
            )
        {
            return;
        }
        let font = (entity.style.text_size.px() * self.camera.scale).max(1.0);
        let (center, width) = match entity.shape {
            CanvasShape::Path | CanvasShape::Freehand => {
                (path_middle(&flattened(&entity.world_points())), f32::INFINITY)
            }
            _ => (
                entity.transform.center,
                ((entity.transform.size.x - TEXT_INSET * 2.0) * self.camera.scale).max(1.0),
            ),
        };
        let galley = painter.layout_text(
            &entity.text,
            FontId::proportional(font),
            TextLayout {
                align: TextAlign::Center,
                ..TextLayout::wrapped(width)
            },
        );
        let size = galley.size();
        let at = self.at(center);
        painter.galley(pos2(at.x - size.x / 2.0, at.y - size.y / 2.0), galley, color);
    }

    fn draw_rectangle(&self, painter: &Painter, color: Color32, width: f32, opacity: f32) {
        let painter = self.turned(painter);
        let rect = self.box_rect();
        let radius = (self.entity.style.corner_radius * self.camera.scale).max(0.0);
        if let Some(fill) = self.entity.style.filled() {
            let fill = with_opacity(resolve_color(fill, self.palette.auto), opacity);
            painter.rect_filled(rect, radius, fill);
        }
        match self.entity.style.stroke_style {
            CanvasStrokeStyle::Solid => painter.rect_stroke(rect, radius, width, color),
            stroke_style => {
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ];
                stroke_polyline(&painter, &corners, width, color, stroke_style);
            }
        }
    }

    fn draw_pen(&self, painter: &Painter, color: Color32, width: f32) {
        let points: Vec<Pos2> = self
            .entity
            .world_points()
            .iter()
            .map(|point| self.at(point.at()))
            .collect();
        stroke(painter, &points, width, &self.entity.pressures, color);
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

pub(crate) fn path_middle(line: &[CanvasPoint]) -> CanvasPoint {
    let lengths: Vec<f32> = line.windows(2).map(|pair| distance(pair[0], pair[1])).collect();
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

pub(crate) fn arrowhead_size(stroke_width: f32) -> f32 {
    (stroke_width * 4.0).max(8.0)
}

pub(crate) fn styled_polyline(
    painter: &Painter,
    line: &[Pos2],
    width: f32,
    color: Color32,
    style: CanvasStyle,
    zoom: f32,
) {
    let mut shaft = line.to_vec();
    let size = arrowhead_size(style.stroke_width) * zoom;
    let mut heads = Vec::new();
    for (head, end) in [(style.start_arrowhead, false), (style.end_arrowhead, true)] {
        if head == CanvasArrowhead::None || shaft.len() < 2 {
            continue;
        }
        let (tip, before) = match end {
            true => (shaft[shaft.len() - 1], shaft[shaft.len() - 2]),
            false => (shaft[0], shaft[1]),
        };
        let delta = Vec2::new(before.x - tip.x, before.y - tip.y);
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }
        let inward = Vec2::new(delta.x / length, delta.y / length);
        let arrow = size.min(length * 0.4);
        let inset = pos2(tip.x + inward.x * arrow * 0.75, tip.y + inward.y * arrow * 0.75);
        match end {
            true => *shaft.last_mut().unwrap() = inset,
            false => shaft[0] = inset,
        }
        heads.push((tip, inward, arrow));
    }
    stroke_polyline(painter, &shaft, width, color, style.stroke_style);
    for (tip, inward, arrow) in heads {
        arrowhead(painter, tip, inward, arrow, width, color);
    }
}

pub(crate) fn stroke_polyline(
    painter: &Painter,
    line: &[Pos2],
    width: f32,
    color: Color32,
    style: CanvasStrokeStyle,
) {
    let (dash, gap) = match style {
        CanvasStrokeStyle::Solid => {
            for pair in line.windows(2) {
                painter.line(pair[0], pair[1], width, color);
            }
            for joint in line.iter().skip(1).take(line.len().saturating_sub(2)) {
                painter.rect_filled(
                    Rect::from_center_size(*joint, Vec2::splat(width)),
                    width / 2.0,
                    color,
                );
            }
            return;
        }
        CanvasStrokeStyle::Dashed => ((width * 3.0).max(2.0), (width * 2.0).max(2.0)),
        CanvasStrokeStyle::Dotted => (0.0, (width * 2.0).max(2.0)),
    };
    let period = dash + gap;
    let mut offset = 0.0;
    for pair in line.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let delta = Vec2::new(end.x - start.x, end.y - start.y);
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }
        let along = |amount: f32| {
            pos2(
                start.x + delta.x / length * amount,
                start.y + delta.y / length * amount,
            )
        };
        let mut at = (period - offset).rem_euclid(period);
        if offset > 0.0 && offset < dash {
            painter.line(along(0.0), along((dash - offset).min(length)), width, color);
        }
        while at < length {
            match style {
                CanvasStrokeStyle::Dotted => painter.rect_filled(
                    Rect::from_center_size(along(at), Vec2::splat(width)),
                    width / 2.0,
                    color,
                ),
                _ => painter.line(along(at), along((at + dash).min(length)), width, color),
            }
            at += period;
        }
        offset = (offset + length).rem_euclid(period);
    }
}

fn fill_polygon(painter: &Painter, outline: &[Pos2], color: Color32) {
    let Some(top) = outline.iter().map(|point| point.y).reduce(f32::min) else {
        return;
    };
    let bottom = outline.iter().map(|point| point.y).fold(top, f32::max);
    let mut y = top + 0.5;
    while y < bottom {
        let mut crossings: Vec<f32> = outline
            .iter()
            .zip(outline.iter().cycle().skip(1))
            .filter(|(a, b)| (a.y <= y) != (b.y <= y))
            .map(|(a, b)| a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x))
            .collect();
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.chunks(2) {
            if let [left, right] = pair {
                painter.rect_filled(
                    Rect::from_min_max(pos2(*left, y - 0.5), pos2(*right, y + 0.5)),
                    0.0,
                    color,
                );
            }
        }
        y += 1.0;
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

pub(crate) fn stroke(
    painter: &Painter,
    points: &[Pos2],
    width: f32,
    pressures: &[f32],
    color: Color32,
) {
    let width_at = |index: usize| {
        pressures
            .get(index)
            .map_or(width, |pressure| pressed_width(width, *pressure))
    };
    let dot = |at: Pos2, width: f32| {
        painter.rect_filled(
            Rect::from_center_size(at, Vec2::splat(width)),
            width / 2.0,
            color,
        );
    };
    if let [only] = points {
        dot(*only, width_at(0));
    }
    for (index, window) in points.windows(2).enumerate() {
        let segment = (width_at(index) + width_at(index + 1)) / 2.0;
        painter.line(window[0], window[1], segment, color);
        if index > 0 && !pressures.is_empty() {
            dot(window[0], width_at(index));
        }
    }
}
