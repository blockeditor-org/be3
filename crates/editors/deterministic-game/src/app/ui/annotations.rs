use block_editor_beui::beui::{Color32, Modifiers, Painter, Pos2, Rect, Vec2};
use game_api::Spot;

use super::layout::Layout;

const SHAFT: f32 = 0.16;
const HEAD: f32 = 0.34;
const RING: f32 = 0.08;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Brush {
    Green,
    Red,
    Blue,
    Yellow,
}

impl Brush {
    pub(crate) fn of(modifiers: Modifiers) -> Self {
        match (modifiers.shift || modifiers.ctrl, modifiers.alt) {
            (false, false) => Brush::Green,
            (true, false) => Brush::Red,
            (false, true) => Brush::Blue,
            (true, true) => Brush::Yellow,
        }
    }

    fn color(self) -> Color32 {
        match self {
            Brush::Green => Color32::from_rgb(64, 150, 70),
            Brush::Red => Color32::from_rgb(205, 60, 60),
            Brush::Blue => Color32::from_rgb(60, 110, 200),
            Brush::Yellow => Color32::from_rgb(230, 170, 30),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Annotation {
    pub(crate) from: Spot,
    pub(crate) to: Spot,
    pub(crate) brush: Brush,
}

pub(crate) fn toggled(annotations: &[Annotation], drawn: Annotation) -> Vec<Annotation> {
    let mut next: Vec<Annotation> = annotations
        .iter()
        .filter(|kept| (kept.from, kept.to) != (drawn.from, drawn.to))
        .copied()
        .collect();
    if !annotations.contains(&drawn) {
        next.push(drawn);
    }
    next
}

pub(crate) fn paint(
    painter: &Painter,
    rect: Rect,
    layout: &Layout,
    annotations: &[Annotation],
    pending: Option<Annotation>,
) {
    if layout.size.x <= 0.0 || layout.size.y <= 0.0 {
        return;
    }
    let scale = rect.width() / layout.size.x;
    let to_screen = |point: Pos2| rect.min + point.to_vec2() * scale;
    for annotation in annotations.iter().chain(pending.as_ref()) {
        let (Some(from), Some(to)) = (layout.find(annotation.from), layout.find(annotation.to))
        else {
            continue;
        };
        let size = from.rect.width().min(from.rect.height()) * scale;
        let color = annotation.brush.color();
        if annotation.from == annotation.to {
            let ring = Rect::from_center_size(to_screen(from.rect.center()), Vec2::splat(size))
                .shrink(size * RING / 2.0);
            painter.rect_stroke(ring, ring.width() / 2.0, size * RING, color);
            continue;
        }
        arrow(
            painter,
            to_screen(from.rect.center()),
            to_screen(to.rect.center()),
            size,
            color,
        );
    }
}

fn arrow(painter: &Painter, from: Pos2, to: Pos2, size: f32, color: Color32) {
    let along = to - from;
    let length = along.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = along / length;
    let across = Vec2::new(-direction.y, direction.x);
    let head = size * HEAD;
    let neck = to - direction * head;
    painter.line(from + direction * size * 0.2, neck, size * SHAFT, color);
    let steps = (head / 1.5).ceil().max(4.0) as usize;
    let strip = head / steps as f32;
    for step in 0..steps {
        let fraction = (step as f32 + 0.5) / steps as f32;
        let width = head * (1.0 - fraction);
        let at = neck + direction * head * fraction;
        painter.line(at - across * width, at + across * width, strip, color);
    }
}
