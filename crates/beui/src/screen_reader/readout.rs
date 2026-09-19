use crate::color::Color32;
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::painter::Painter;
use crate::styled::Theme;

pub(crate) const HEIGHT: f32 = 52.0;

const PADDING: f32 = 12.0;
const STATUS_SPACING: f32 = 16.0;
const UTTERANCE_SIZE: f32 = 19.0;
const STATUS_SIZE: f32 = 12.0;
const BORDER_WIDTH: f32 = 1.0;
const FOCUS_WIDTH: f32 = 2.0;
const FINGER_RADIUS: f32 = 18.0;
const FINGER_WIDTH: f32 = 2.0;
const FOCUS_FILL: Color32 = Color32::from_rgba_unmultiplied(82, 137, 255, 72);

pub(super) struct View<'a> {
    pub(super) bar: Rect,
    pub(super) spoken: Option<&'a str>,
    pub(super) status: String,
    pub(super) finger: Option<Pos2>,
}

pub(super) fn paint_focus(painter: &Painter, scale: f32, focus: Rect) -> Rect {
    let content = painter.clip_rect();
    let rect = focus.scaled(scale).intersect(content);
    if !content.is_positive() || !rect.is_positive() {
        return Rect::NOTHING;
    }
    painter.rect_filled(rect, 0.0, FOCUS_FILL);
    painter.rect_stroke(rect, 0.0, FOCUS_WIDTH, Theme::DARK.accent);
    rect.expand(FOCUS_WIDTH)
}

pub(super) fn paint(painter: &Painter, scale: f32, view: &View<'_>) -> Rect {
    let mut painted = bar(painter, view);
    if let Some(at) = view.finger.map(|at| pos2(at.x * scale, at.y * scale)) {
        let ring = Rect::from_min_size(
            at - vec2(FINGER_RADIUS, FINGER_RADIUS),
            Vec2::splat(FINGER_RADIUS * 2.0),
        );
        painter.rect_stroke(ring, FINGER_RADIUS, FINGER_WIDTH, Theme::DARK.knob);
        painted = painted.union(ring);
    }
    painted
}

fn bar(painter: &Painter, view: &View<'_>) -> Rect {
    let bar = view.bar;
    if !bar.is_positive() {
        return Rect::NOTHING;
    }
    painter.rect_filled(bar, 0.0, Theme::DARK.background);
    let border = Rect::from_min_max(bar.min, pos2(bar.right(), bar.top() + BORDER_WIDTH));
    painter.rect_filled(border, 0.0, Theme::DARK.border);
    let inner = bar.shrink(PADDING);
    if !inner.is_positive() {
        return bar;
    }
    let status = painter.layout(
        view.status.clone(),
        FontId::monospace(STATUS_SIZE),
        f32::INFINITY,
    );
    let status_size = status.size();
    let spoken = painter.layout(
        view.spoken.unwrap_or_default().to_owned(),
        FontId::proportional(UTTERANCE_SIZE),
        f32::INFINITY,
    );
    let spoken_size = spoken.size();
    let words = Rect::from_min_max(
        inner.min,
        pos2(
            (inner.right() - status_size.x - STATUS_SPACING).max(inner.left()),
            inner.bottom(),
        ),
    );
    painter.with_clip_rect(words).galley(
        pos2(words.left(), inner.center().y - spoken_size.y / 2.0),
        spoken,
        Theme::DARK.text,
    );
    painter.galley(
        pos2(
            inner.right() - status_size.x,
            inner.center().y - status_size.y / 2.0,
        ),
        status,
        Theme::DARK.accent,
    );
    bar
}
