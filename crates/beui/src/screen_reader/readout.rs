use crate::color::Color32;
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::painter::Painter;
use crate::styled::Theme;

const BAR_PADDING: f32 = 12.0;
const BAR_HEIGHT: f32 = 52.0;
const STATUS_SPACING: f32 = 16.0;
const UTTERANCE_SIZE: f32 = 19.0;
const STATUS_SIZE: f32 = 12.0;
const FOCUS_WIDTH: f32 = 2.0;
const FINGER_RADIUS: f32 = 18.0;
const FINGER_WIDTH: f32 = 2.0;
const BAR_FILL: Color32 = Color32::from_rgba_unmultiplied(14, 17, 23, 232);
const FOCUS_FILL: Color32 = Color32::from_rgba_unmultiplied(82, 137, 255, 72);

pub(super) struct View<'a> {
    pub(super) content: Rect,
    pub(super) reserved: f32,
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
    if !view.content.is_positive() {
        return Rect::NOTHING;
    }
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
    let content = view.content;
    let status = painter.layout(
        view.status.clone(),
        FontId::monospace(STATUS_SIZE),
        f32::INFINITY,
    );
    let width = content.width() - BAR_PADDING * 2.0 - status.size().x - STATUS_SPACING;
    if width <= 0.0 {
        return Rect::NOTHING;
    }
    let spoken = painter.layout(
        view.spoken.unwrap_or_default().to_owned(),
        FontId::proportional(UTTERANCE_SIZE),
        width,
    );
    let height = (spoken.size().y + BAR_PADDING * 2.0)
        .max(BAR_HEIGHT)
        .min(content.height());
    let lift = view
        .reserved
        .clamp(0.0, (content.height() - height).max(0.0));
    let bottom = content.bottom() - lift;
    let bar = Rect::from_min_max(
        pos2(content.left(), bottom - height),
        pos2(content.right(), bottom),
    );
    painter.rect_filled(bar, 0.0, BAR_FILL);
    let inner = bar.shrink(BAR_PADDING);
    let spoken_size = spoken.size();
    painter.galley(
        pos2(inner.left(), inner.center().y - spoken_size.y / 2.0),
        spoken,
        Theme::DARK.text,
    );
    let status_size = status.size();
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
