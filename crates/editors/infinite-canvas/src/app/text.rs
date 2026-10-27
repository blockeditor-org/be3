use std::cell::Cell;
use std::sync::Arc;

use block_editor_beui::be_block::be_model::ObjectId;
use block_editor_beui::beui::native::with_document;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    Color32, Key, KeyAt, KeyPhase, NodeId, Rect, TextAlign, Vec2, line_height, pos2,
};
use block_editor_beui::live_text::{FieldOf, LiveText, Synced};
use block_editor_beui::{SharedCaret, remote_carets};
use infinite_canvas_types::{
    Canvas, CanvasEntity, CanvasPoint, CanvasShape, CanvasTextAlign, CanvasTextStyle,
    CanvasTextVerticalAlign, CanvasTextWeight, Entity,
};
use text_editor_core::schema::basic::Basic;
use text_editor_core::{EditorCommand, TextBuffer};
use uuid::Uuid;

use crate::geometry::{TEXT_INSET, WorldRect, flattened};

use super::paint::{path_middle, resolve_color, with_opacity};
use super::state::{CanvasState, TextEntry};
use beui::Sides;

pub(crate) fn font_px(entity: &CanvasEntity, scale: f32) -> f32 {
    (entity.style.text_size.px() * scale).clamp(4.0, 256.0)
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

pub(crate) fn labels(shape: CanvasShape) -> bool {
    matches!(
        shape,
        CanvasShape::Rectangle | CanvasShape::Ellipse | CanvasShape::Diamond | CanvasShape::Path
    )
}

pub(crate) fn writes(entity: &CanvasEntity) -> bool {
    !entity.hidden && (entity.shape == CanvasShape::Text || labels(entity.shape))
}

pub(crate) fn text_style(entity: &CanvasEntity) -> CanvasTextStyle {
    match entity.shape {
        CanvasShape::Text => entity.text_style,
        CanvasShape::Path => CanvasTextStyle {
            alignment: CanvasTextAlign::Center,
            vertical_alignment: CanvasTextVerticalAlign::Middle,
            wrap: false,
            ..entity.text_style
        },
        _ => CanvasTextStyle {
            alignment: CanvasTextAlign::Center,
            vertical_alignment: CanvasTextVerticalAlign::Middle,
            wrap: true,
            ..entity.text_style
        },
    }
}

pub(crate) fn rich_state(entity: &CanvasEntity) -> TextAreaState {
    let buffer = TextBuffer::rich(entity.text.bytes(), entity.text.seeds());
    TextAreaState::with_schema(Arc::new(buffer), Arc::new(Basic))
}

pub(crate) fn text_turn(entity: &CanvasEntity) -> f32 {
    match entity.shape {
        CanvasShape::Path | CanvasShape::Freehand => 0.0,
        _ => entity.transform.rotation,
    }
}

pub(crate) fn text_box(entity: &CanvasEntity) -> Rect {
    let center = entity.transform.center;
    let size = entity.transform.size;
    Rect::from_min_max(
        pos2(center.x - size.x / 2.0, center.y - size.y / 2.0),
        pos2(center.x + size.x / 2.0, center.y + size.y / 2.0),
    )
}

pub(crate) fn text_region(entity: &CanvasEntity) -> Rect {
    match entity.shape {
        CanvasShape::Path | CanvasShape::Freehand => {
            let reach = path_label_reach(entity);
            Rect::from_min_max(
                pos2(reach.min.x, reach.min.y),
                pos2(reach.max.x, reach.max.y),
            )
        }
        _ => text_box(entity),
    }
}

pub(crate) fn path_label_reach(entity: &CanvasEntity) -> WorldRect {
    let px = entity.style.text_size.px();
    let text = entity.text.to_str_lossy();
    let lines = text.lines().count().max(1) as f32;
    let widest = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32;
    let half = CanvasPoint::new(widest * px * 0.35 + px, lines * px * 0.75 + px);
    let center = path_middle(&flattened(&entity.world_points()));
    WorldRect::from_points(
        CanvasPoint::new(center.x - half.x, center.y - half.y),
        CanvasPoint::new(center.x + half.x, center.y + half.y),
    )
}

#[derive(Clone, PartialEq)]
struct Shown {
    placeholder: String,
    text_style: CanvasTextStyle,
    size: f32,
    color: Color32,
    locked: bool,
    region: Rect,
    turn: f32,
}

#[component]
pub(crate) fn EntityText(
    state: CanvasState,
    id: Uuid,
    entity: Memo<Option<CanvasEntity>>,
    outer: Memo<Rect>,
    editing: Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let shown = create_memo(move || {
        let entity = entity.get().filter(writes)?;
        Some(Shown {
            size: font_px(&entity, 1.0),
            placeholder: entity.placeholder.clone(),
            text_style: text_style(&entity),
            color: with_opacity(
                resolve_color(entity.style.stroke_color, beui::reactive::resolve_color(theme.text.get())),
                entity.style.opacity,
            ),
            locked: entity.locked,
            region: text_region(&entity),
            turn: text_turn(&entity),
        })
    });
    let placeholder = create_memo(move || {
        shown
            .get()
            .map(|shown| shown.placeholder)
            .unwrap_or_default()
    });
    let color = create_memo(move || {
        TextColor::Custom((
            shown
                .get()
                .map_or(Color32::TRANSPARENT, |shown| shown.color)
        ).into())
    });
    let font_size = create_memo(move || shown.get().map_or(20.0, |shown| shown.size));
    let size = create_memo(move || TextSize::Custom(font_size.get()));
    let prose = create_memo(move || {
        let text_style = shown
            .get()
            .map(|shown| shown.text_style)
            .unwrap_or_default();
        Some(prose(text_style, font_size.get()))
    });
    let disabled =
        create_memo(move || !editing.get() || shown.get().is_none_or(|shown| shown.locked));
    let turn = create_memo(move || match editing.get() {
        true => 0.0,
        false => shown.get().map_or(0.0, |shown| shown.turn),
    });
    let padding = create_memo(move || {
        let (outer, inner) = (
            outer.get(),
            shown.get().map_or(Rect::ZERO, |shown| shown.region),
        );
        Padding::custom(Sides::new(
            (inner.left() - outer.left()).max(0.0),
            (inner.top() - outer.top()).max(0.0),
            (outer.right() - inner.right()).max(0.0),
            (outer.bottom() - inner.bottom()).max(0.0),
        ))
    });
    let field = NodeRef::new();
    let grows = create_signal(false);
    let area = live_text(state, id, editing, grows);
    let content = create_signal(Vec2::ZERO);
    create_effect(move || {
        if !editing.get() {
            grows.set(false);
        }
    });
    create_effect(move || {
        let content = content.get();
        if !editing.get() || !grows.get() || content == Vec2::ZERO {
            return;
        }
        untrack(|| {
            if state.fit_edited_text(id, content)
                && let Some(node) = field.try_get()
            {
                node.reveal();
            }
        });
    });
    let remote_cursors = peer_carets(state, id, area);
    share_caret(state, id, area, editing);
    create_effect(move || {
        if !editing.get() {
            return;
        }
        untrack(|| {
            if let Some(entry) = state.take_text_entry() {
                with_document(|document| {
                    document.report_after_layout(move || enter(area, field, entry))
                });
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
    let left = move |has_focus: bool| {
        if !has_focus && editing.get_untracked() {
            state.finish_text();
        }
    };
    view! {
        <Frame padding={padding}>
            <Frame rotate={turn}>
                <TextArea
                    @node_ref=&field
                    @test_id={format!("infinite-canvas.text.{id}")}
                    state={area}
                    line_numbers=false
                    find=false
                    color
                    size
                    padding=TEXT_PADDING
                    placeholder
                    disabled
                    focused={editing}
                    prose
                    remote_cursors
                    on_content_size={move |size: Vec2| content.set(size)}
                    on_focus_change={left}
                />
            </Frame>
        </Frame>
    }
}

const TEXT_PADDING: Padding = Padding::all(Space::Custom(TEXT_INSET));

fn peer_carets(state: CanvasState, id: Uuid, area: TextAreaState) -> Memo<Vec<RemoteTextCursor>> {
    let carets = state.peer_carets();
    let theirs = create_memo(move || {
        carets.with(|carets| {
            carets
                .iter()
                .filter(|(caret, _)| caret.entity == id)
                .map(|(caret, color)| (caret.caret, *color))
                .collect::<Vec<_>>()
        })
    });
    create_memo(move || {
        let theirs = theirs.get();
        if theirs.is_empty() {
            return Vec::new();
        }
        area.content().get();
        remote_carets(area, theirs)
    })
}

fn share_caret(state: CanvasState, id: Uuid, area: TextAreaState, editing: Memo<bool>) {
    create_effect(move || {
        let caret = match editing.get() {
            true => {
                area.cursors().get();
                area.content().get();
                untrack(|| SharedCaret::of(area))
            }
            false => None,
        };
        untrack(|| state.note_caret(id, caret));
    });
    on_cleanup(move || {
        if state.is_alive() {
            state.note_caret(id, None);
        }
    });
}

fn live_text(
    state: CanvasState,
    id: Uuid,
    editing: Memo<bool>,
    grows: Signal<bool>,
) -> TextAreaState {
    let object = ObjectId::from_uuid(id);
    let editor = state.editor();
    let host = editor.host();
    let content = state.content();
    let document = Arc::new(LiveText::new(
        FieldOf::<Canvas>::rich(object, Entity::TEXT),
        host.waker(),
    ));
    let _ = untrack(|| content.read(|content| document.adopt(content)));
    document.take_external_edit();
    let area = TextAreaState::with_schema(
        Arc::clone(&document) as Arc<dyn text_editor_core::Document>,
        Arc::new(Basic),
    );
    let stored = content.field(object, Entity::TEXT);
    let adopted = store(Cell::new(false));
    let fitted = store(Cell::new(0_u64));
    create_effect(move || {
        area.content().get();
        let editing = editing.get();
        match editing {
            true => {
                let _ = content.revision();
            }
            false => stored.with(|_| ()),
        }
        untrack(|| {
            let Some(adopted) = adopted.try_value() else {
                return;
            };
            if !editing {
                if adopted.replace(false) {
                    document.sync(&content, &editor.host(), state.block_id(), false);
                }
                let _ = content.read(|content| document.adopt(content));
                if document.take_external_edit() {
                    area.external_edit();
                }
                return;
            }
            let first = !adopted.get();
            match document.sync(&content, &editor.host(), state.block_id(), first) {
                Synced::Waiting => return,
                Synced::External => area.external_edit(),
                Synced::Unchanged => area.sync(),
            }
            adopted.set(true);
            let revision = text_editor_core::Document::revision(document.as_ref());
            let fitted = fitted.value();
            let changed = match first {
                true => {
                    fitted.set(revision);
                    area.bytes().is_empty()
                }
                false => fitted.replace(revision) != revision,
            };
            if changed && !grows.get_untracked() {
                grows.set(true);
            }
        });
    });
    area
}

fn enter(area: TextAreaState, field: NodeRef, entry: TextEntry) {
    let Some(node) = field.try_get() else {
        return;
    };
    let range = match entry {
        TextEntry::All => {
            area.execute(EditorCommand::SelectAll);
            return;
        }
        TextEntry::At(at) => {
            let at = with_document(|document| {
                let surface = document.api::<TextAreaSurfaceApi>(node).surface();
                document.api::<TextAreaApi>(surface).index_at(document, at)
            });
            at..at
        }
    };
    area.select(range);
}
