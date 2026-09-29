use std::rc::Rc;

use block_editor_beui::be_block::canvas::{CanvasEntity, CanvasEntityKind};
use block_editor_beui::beui::icons::{ICON_VISIBILITY, ICON_VISIBILITY_OFF};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, clone, component, component_rect,
    create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Accordion, Body, ButtonVariant, Caption, IconButton, IconButtonSize, use_theme,
};
use block_editor_beui::beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};
use block_editor_beui::beui::{Color32, CursorIcon, NodeId};
use uuid::Uuid;

use crate::geometry::entity_kind_label;

use super::state::CanvasState;

const ROW_PADDING: f32 = 4.0;
const ROW_RADIUS: u8 = 4;

#[derive(Clone, Copy, PartialEq)]
struct Layer(Uuid);

#[component]
pub(crate) fn LayersSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let ids = create_memo(clone!(state -> move || {
        state
            .entities
            .get()
            .iter()
            .rev()
            .map(|entity| entity.id)
            .collect::<Vec<Uuid>>()
    }));
    let empty = create_memo(clone!(ids -> move || ids.with(Vec::is_empty)));
    view! {
        <Accordion title="Layers" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=2.0 @test_id={"infinite-canvas.layers"}>
                <Show condition={empty}>
                    <Caption content="Nothing on the canvas yet." wrap=true />
                </Show>
                <ForEach keys={ids}>
                    {move |id: Uuid| {
                        let state = Rc::clone(&state);
                        view! {
                            <LayerRow state id />
                        }
                    }}
                </ForEach>
            </List>
        </Accordion>
    }
}

#[component]
fn LayerRow(state: Rc<CanvasState>, id: Uuid) -> NodeId {
    let rect = component_rect();
    let entity = create_memo(clone!(state -> move || {
        state.entities.get().into_iter().find(|entity| entity.id == id)
    }));
    let label = create_memo(clone!(state entity -> move || {
        entity
            .get()
            .map(|entity| layer_label(&state, &entity))
            .unwrap_or_default()
    }));
    let selected = create_memo(clone!(state -> move || state.selection.get().contains(&id)));
    let hidden = create_memo(clone!(entity -> move || {
        entity.get().is_some_and(|entity| entity.style.hidden)
    }));
    let glyph = create_memo(clone!(hidden -> move || match hidden.get() {
        true => ICON_VISIBILITY_OFF.to_owned(),
        false => ICON_VISIBILITY.to_owned(),
    }));
    let toggle_label = create_memo(clone!(hidden -> move || match hidden.get() {
        true => "Show".to_owned(),
        false => "Hide".to_owned(),
    }));
    let theme = use_theme();
    let text = create_memo(clone!(theme hidden -> move || match hidden.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let toggle = clone!(state hidden -> move || state.set_hidden(id, !hidden.get_untracked()));
    let select = clone!(state -> move || state.select(id, false));
    let dropped = clone!(state -> move |(Layer(moved), point): (Layer, DragPoint)| {
        let in_front = point.pos.y < rect.get_untracked().center().y;
        state.move_layer(moved, id, in_front);
    });
    let ghost = clone!(label -> move |_: Layer| view! {
        <LayerGhost label={label.clone()} />
    });
    view! {
        <DropTarget on_drop={dropped}>
            {move |handle: DropHandle| {
                let fill = create_memo(clone!(theme selected -> move || match selected.get() {
                    true => theme.accent_soft.get(),
                    false => Color32::TRANSPARENT,
                }));
                view! {
                    <Frame
                        color={fill}
                        radius=ROW_RADIUS
                        outline={theme.accent.clone()}
                        outline_width=1.0
                        outline_visible={handle.over}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                            <IconButton
                                glyph={glyph.clone()}
                                label={toggle_label.clone()}
                                size=IconButtonSize::Compact
                                variant=ButtonVariant::Ghost
                                capture_presses=true
                                @test_id={format!("infinite-canvas.layer.{id}.visibility")}
                                on_click={toggle.clone()}
                            />
                            <Draggable
                                @sizing=ItemSize::Percent(100.0)
                                payload={Some(Layer(id))}
                                cursor=CursorIcon::PointingHand
                                preview={ghost.clone()}
                                on_click={select.clone()}
                                @test_id={format!("infinite-canvas.layer.{id}")}
                            >
                                {move |_: DragHandle| view! {
                                    <Frame padding_vertical=ROW_PADDING>
                                        <Body content={label.clone()} color={text.clone()} />
                                    </Frame>
                                }}
                            </Draggable>
                        </List>
                    </Frame>
                }
            }}
        </DropTarget>
    }
}

#[component]
fn LayerGhost(label: block_editor_beui::beui::reactive::Memo<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            radius=ROW_RADIUS
            padding_horizontal=8.0
            padding_vertical=ROW_PADDING
        >
            <Body content={label} />
        </Frame>
    }
}

fn layer_label(state: &CanvasState, entity: &CanvasEntity) -> String {
    match &entity.kind {
        CanvasEntityKind::Text {
            text, placeholder, ..
        } => match text.lines().next().filter(|line| !line.trim().is_empty()) {
            Some(line) => line.to_owned(),
            None => placeholder.clone(),
        },
        CanvasEntityKind::Artboard { name } => name.clone(),
        CanvasEntityKind::Block { block_id } | CanvasEntityKind::DirectEditor { block_id, .. } => {
            state
                .label_of(*block_id)
                .map(|label| label.name)
                .unwrap_or_else(|| entity_kind_label(&entity.kind).to_owned())
        }
        kind => entity_kind_label(kind).to_owned(),
    }
}
