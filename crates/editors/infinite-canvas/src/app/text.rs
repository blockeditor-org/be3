use block_editor_beui::be_block::canvas::{
    CanvasEntity, CanvasEntityKind, CanvasTextAlign, CanvasTextStyle, CanvasTextVerticalAlign,
    CanvasTextWeight,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::text_input::text_input_style;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, Prose, TextInputStyle};
use block_editor_beui::beui::{
    Color32, Key, KeyAt, KeyPhase, NodeId, Rect, TextAlign, Vec2, line_height, pos2,
};

use uuid::Uuid;

use crate::geometry::TEXT_INSET;

use super::paint::{resolve_color, with_opacity};
use super::state::{CanvasState, TextEntry};

const PLACEHOLDER_OPACITY: f32 = 0.4;

pub(crate) fn font_px(text_style: CanvasTextStyle, scale: f32) -> f32 {
    (text_style.font_size * scale).clamp(4.0, 256.0)
}

pub(crate) fn prose(text_style: CanvasTextStyle, font_px: f32) -> Prose {
    let gap = line_height(font_px) * (text_style.line_height.max(0.5) - 1.0) / 2.0;
    Prose {
        bold: text_style.weight == CanvasTextWeight::Bold,
        align: match text_style.alignment {
            CanvasTextAlign::Left => TextAlign::Start,
            CanvasTextAlign::Center => TextAlign::Center,
            CanvasTextAlign::Right => TextAlign::End,
        },
        vertical_align: match text_style.vertical_alignment {
            CanvasTextVerticalAlign::Top => TextAlign::Start,
            CanvasTextVerticalAlign::Middle => TextAlign::Center,
            CanvasTextVerticalAlign::Bottom => TextAlign::End,
        },
        line_padding: (gap, gap),
        wrap: text_style.wrap,
    }
}

pub(crate) fn measure(entity: &CanvasEntity, scale: f32) -> Option<Vec2> {
    let CanvasEntityKind::Text {
        text,
        text_style,
        placeholder,
    } = &entity.kind
    else {
        return None;
    };
    let font_px = font_px(*text_style, scale);
    let wrap_width = (entity.transform.size.x - TEXT_INSET * 2.0) * scale;
    let measured = unstyled::measure_prose(
        text,
        placeholder,
        font_px,
        prose(*text_style, font_px),
        wrap_width,
    )?;
    Some(Vec2::new(measured.x / scale, measured.y / scale))
}

pub(crate) fn shown_live(entity: &CanvasEntity) -> bool {
    matches!(entity.kind, CanvasEntityKind::Text { .. }) && !entity.style.hidden
}

pub(crate) fn text_box(entity: &CanvasEntity) -> Rect {
    let center = entity.transform.center;
    let size = entity.transform.size;
    Rect::from_min_max(
        pos2(center.x - size.x / 2.0, center.y - size.y / 2.0),
        pos2(center.x + size.x / 2.0, center.y + size.y / 2.0),
    )
}

#[derive(Clone, PartialEq)]
struct Shown {
    text: String,
    placeholder: String,
    text_style: CanvasTextStyle,
    color: Color32,
    locked: bool,
}

#[component]
pub(crate) fn EntityText(
    state: CanvasState,
    id: Uuid,
    entity: Memo<Option<CanvasEntity>>,
    outer: Memo<Rect>,
    camera: Memo<CanvasView>,
    editing: Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let scale = create_memo(move || camera.get().scale.max(f32::EPSILON));
    let shown = create_memo(move || {
        let entity = entity.get()?;
        let CanvasEntityKind::Text {
            text,
            text_style,
            placeholder,
        } = entity.kind
        else {
            return None;
        };
        Some(Shown {
            text,
            placeholder,
            text_style,
            color: with_opacity(
                resolve_color(entity.style.foreground, theme.text.get()),
                entity.style.opacity,
            ),
            locked: entity.locked,
        })
    });
    let text = create_memo(move || shown.get().map(|shown| shown.text).unwrap_or_default());
    let placeholder = create_memo(move || {
        shown
            .get()
            .map(|shown| shown.placeholder)
            .unwrap_or_default()
    });
    let text_style = create_memo(move || {
        shown
            .get()
            .map(|shown| shown.text_style)
            .unwrap_or_default()
    });
    let color = create_memo(move || {
        shown
            .get()
            .map_or(Color32::TRANSPARENT, |shown| shown.color)
    });
    let font_size = create_memo(move || font_px(text_style.get(), scale.get()));
    let disabled =
        create_memo(move || !editing.get() || shown.get().is_none_or(|shown| shown.locked));
    let padding = create_memo(move || {
        let (outer, inner, scale) = (
            outer.get(),
            entity.get().as_ref().map_or(Rect::ZERO, text_box),
            scale.get(),
        );
        Sides::new(
            ((inner.left() - outer.left()) * scale).max(0.0),
            ((inner.top() - outer.top()) * scale).max(0.0),
            ((outer.right() - inner.right()) * scale).max(0.0),
            ((outer.bottom() - inner.bottom()) * scale).max(0.0),
        )
    });
    let style = TextInputStyle {
        font_size: font_size.into_prop(),
        color: color.into_prop(),
        placeholder_color: Prop::dynamic(move || with_opacity(color.get(), PLACEHOLDER_OPACITY)),
        padding_horizontal: Prop::dynamic(move || TEXT_INSET * scale.get()),
        padding_vertical: Prop::dynamic(move || TEXT_INSET * scale.get()),
        prose: Prop::dynamic(move || Some(prose(text_style.get(), font_size.get()))),
        ..text_input_style()
    };
    let field = NodeRef::new();
    create_effect(move || {
        if !editing.get() {
            return;
        }
        untrack(|| {
            if let (Some(entry), Some(node)) = (state.take_text_entry(), field.try_get()) {
                enter(node, entry);
            }
        });
    });
    on_key(KeyAt::Node(field), KeyPhase::Bubble, move |event| {
        let escaped = event
            .press()
            .is_some_and(|press| press.pressed && press.key == Key::Escape);
        if !escaped || !editing.get_untracked() {
            return false;
        }
        state.finish_text();
        state.focus_surface();
        true
    });
    let changed = move |typed: String| {
        let Some(held) = entity.get_untracked() else {
            return;
        };
        let CanvasEntityKind::Text {
            text_style,
            placeholder,
            text,
        } = held.kind.clone()
        else {
            return;
        };
        if text == typed {
            return;
        }
        let mut updated = held.clone();
        updated.kind = CanvasEntityKind::Text {
            text: typed,
            text_style,
            placeholder,
        };
        let updated = state.fit_text(updated);
        state.record_update(vec![held], vec![updated], true);
    };
    let left = move |has_focus: bool| {
        if !has_focus && editing.get_untracked() {
            state.finish_text();
        }
    };
    view! {
        <Frame padding={padding}>
            <unstyled::TextInput
                @node_ref=&field
                @test_id={format!("infinite-canvas.text.{id}")}
                value={Binding::new(text, changed)}
                placeholder={placeholder}
                label="Text"
                disabled={disabled}
                focused={editing}
                multiline=true
                style={style}
                on_focus_change={left}
            />
        </Frame>
    }
}

fn enter(node: NodeId, entry: TextEntry) {
    let (field, range) = with_document(|document| {
        let field = unstyled::text_input_state(document, node);
        let range = match entry {
            TextEntry::All => 0..unstyled::text_input_value(document, node).len(),
            TextEntry::At(at) => {
                let at = unstyled::text_input_index_at(document, node, at);
                at..at
            }
        };
        (field, range)
    });
    field.select(range);
}
