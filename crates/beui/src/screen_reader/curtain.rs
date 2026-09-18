use crate::color::Color32;
use crate::document::Document;
use crate::font::{FontId, Galley};
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::painter::{Painter, Shape};
use crate::styled::Theme;
use crate::styled::theme::CARD_RADIUS;

const MARGIN: f32 = 24.0;
const CARD_WIDTH: f32 = 520.0;
const CARD_PADDING: f32 = 18.0;
const UTTERANCE_SIZE: f32 = 21.0;
const HISTORY_SIZE: f32 = 13.0;
const STATUS_SIZE: f32 = 12.0;
const GUIDE_SIZE: f32 = 12.0;
const LINE_SPACING: f32 = 5.0;
const BLOCK_SPACING: f32 = 14.0;
const FROST_BLEED: f32 = 2.0;
const FROST_RADIUS: f32 = 3.0;
const FOCUS_WIDTH: f32 = 2.0;
const FINGER_RADIUS: f32 = 18.0;
const FINGER_WIDTH: f32 = 2.0;
const CARD_FILL: Color32 = Color32::from_rgba_unmultiplied(20, 24, 32, 236);
const FOCUS_FILL: Color32 = Color32::from_rgba_unmultiplied(82, 137, 255, 72);
const KEYBOARD_GUIDE: [&str; 4] = [
    "Arrows: previous or next item   Tab: previous or next control",
    "Enter or Space: activate        R: repeat",
    "Home or End: first or last      Minus or Plus: adjust",
    "Page Up or Page Down: scroll",
];
const TOUCH_GUIDE: [&str; 5] = [
    "Drag a finger: read what is under it",
    "Flick sideways: previous or next item",
    "Flick up or down: adjust the value",
    "Double tap: activate            Two finger tap: repeat",
    "Drag two fingers: scroll",
];

pub(super) struct View<'a> {
    pub(super) content: Rect,
    pub(super) opacity: f32,
    pub(super) frost: bool,
    pub(super) spoken: &'a [String],
    pub(super) status: String,
    pub(super) focus: Option<Rect>,
    pub(super) finger: Option<Pos2>,
}

struct Line {
    galley: Galley,
    color: Color32,
    space: f32,
}

pub(super) fn paint(painter: &Painter, target: &Document, scale: f32, view: &View<'_>) -> Rect {
    let content = view.content;
    if !content.is_positive() {
        return Rect::NOTHING;
    }
    if view.frost {
        frost(painter, target, scale, content);
    }
    if let Some(rect) = view.focus.map(|rect| rect.scaled(scale).intersect(content))
        && rect.is_positive()
    {
        painter.rect_filled(rect, 0.0, FOCUS_FILL);
        painter.rect_stroke(rect, 0.0, FOCUS_WIDTH, Theme::DARK.accent);
    }
    painter.rect_filled(content, 0.0, curtain_color(view.opacity));
    let mut painted = content;
    if let Some(at) = view.finger.map(|at| pos2(at.x * scale, at.y * scale)) {
        let ring = Rect::from_min_size(
            at - vec2(FINGER_RADIUS, FINGER_RADIUS),
            Vec2::splat(FINGER_RADIUS * 2.0),
        );
        painter.rect_stroke(ring, FINGER_RADIUS, FINGER_WIDTH, Theme::DARK.knob);
        painted = painted.union(ring);
    }
    painted.union(speech(painter, view, content))
}

fn speech(painter: &Painter, view: &View<'_>, content: Rect) -> Rect {
    let width = (content.width() - (MARGIN + CARD_PADDING) * 2.0).min(CARD_WIDTH);
    if width <= 0.0 {
        return Rect::NOTHING;
    }
    let mut lines = Vec::new();
    let (current, history) = view
        .spoken
        .split_last()
        .map_or((None, &[][..]), |(last, rest)| (Some(last), rest));
    for line in history {
        lines.push(Line {
            galley: painter.layout(line.clone(), FontId::proportional(HISTORY_SIZE), width),
            color: Theme::DARK.text_muted,
            space: LINE_SPACING,
        });
    }
    lines.push(Line {
        galley: painter.layout(
            current.cloned().unwrap_or_default(),
            FontId::proportional(UTTERANCE_SIZE),
            width,
        ),
        color: Theme::DARK.text,
        space: BLOCK_SPACING,
    });
    lines.push(Line {
        galley: painter.layout(view.status.clone(), FontId::monospace(STATUS_SIZE), width),
        color: Theme::DARK.accent,
        space: BLOCK_SPACING,
    });
    for group in [KEYBOARD_GUIDE.as_slice(), TOUCH_GUIDE.as_slice()] {
        for (index, guide) in group.iter().enumerate() {
            lines.push(Line {
                galley: painter.layout((*guide).to_owned(), FontId::monospace(GUIDE_SIZE), width),
                color: Theme::DARK.text_muted,
                space: if index == 0 {
                    BLOCK_SPACING
                } else {
                    LINE_SPACING
                },
            });
        }
    }
    let height: f32 = lines
        .iter()
        .map(|line| line.galley.size().y + line.space)
        .sum::<f32>()
        - lines.last().map_or(0.0, |line| line.space);
    let card = Rect::from_min_size(
        pos2(
            content.center().x - width / 2.0,
            content.center().y - height / 2.0,
        ),
        vec2(width, height),
    );
    let panel = card.expand(CARD_PADDING);
    painter.rect_filled(panel, f32::from(CARD_RADIUS), CARD_FILL);
    painter.rect_stroke(panel, f32::from(CARD_RADIUS), 1.0, Theme::DARK.border);
    let mut top = card.top();
    for line in lines {
        let size = line.galley.size().y;
        painter.galley(pos2(card.left(), top), line.galley, line.color);
        top += size + line.space;
    }
    panel
}

fn frost(painter: &Painter, target: &Document, scale: f32, content: Rect) {
    let color = opaque(target.theme().text_muted);
    for shape in target.shapes() {
        let Shape::Text {
            origin,
            galley,
            clip,
            ..
        } = shape
        else {
            continue;
        };
        let words = Rect::from_min_size(*origin, galley.size()).intersect(*clip);
        let block = words.scaled(scale).expand(FROST_BLEED).intersect(content);
        if block.is_positive() {
            painter.rect_filled(block, FROST_RADIUS, color);
        }
    }
}

fn opaque(color: Color32) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgb(red, green, blue)
}

fn curtain_color(opacity: f32) -> Color32 {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(0, 0, 0, alpha)
}
