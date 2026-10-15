use block_editor_beui::be_block::canvas::{CanvasEntity, CanvasEntityKind};
use block_editor_beui::beui::icons::{ICON_VISIBILITY, ICON_VISIBILITY_OFF};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};
use block_editor_beui::beui::{Color32, CursorIcon, NodeId};
use uuid::Uuid;

use crate::geometry::entity_kind_label;

use super::state::CanvasState;

const ROW_PADDING: f32 = 4.0;
const ROW_RADIUS: f32 = 4.0;

#[derive(Clone, Copy, PartialEq)]
struct Layer(Uuid);

#[component]
pub(crate) fn LayersSection(state: CanvasState) -> NodeId {
    let (open, set_open) = create_signal(true);
    let ids = create_memo(move || {
        state
            .entities
            .get()
            .iter()
            .rev()
            .map(|entity| entity.id)
            .collect::<Vec<Uuid>>()
    });
    let empty = create_memo(move || ids.with(Vec::is_empty));
    view! {
        <Accordion title="Layers" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=2.0 @test_id={"infinite-canvas.layers"}>
                <Show condition={empty}>
                    <Text style=TextStyle::CAPTION string="Nothing on the canvas yet." wrap=true />
                </Show>
                <ForEach keys={ids}>
                    {move |id: Uuid| {
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
fn LayerRow(state: CanvasState, id: Uuid) -> NodeId {
    let entity = create_memo(move || {
        state
            .entities
            .get()
            .into_iter()
            .find(|entity| entity.id == id)
    });
    let label = create_memo(move || {
        entity
            .get()
            .map(|entity| layer_label(&state, &entity))
            .unwrap_or_default()
    });
    let selected = create_memo(move || state.selection.get().contains(&id));
    let hidden = create_memo(move || entity.get().is_some_and(|entity| entity.style.hidden));
    let glyph = create_memo(move || match hidden.get() {
        true => ICON_VISIBILITY_OFF.to_owned(),
        false => ICON_VISIBILITY.to_owned(),
    });
    let toggle_label = create_memo(move || match hidden.get() {
        true => "Show".to_owned(),
        false => "Hide".to_owned(),
    });
    let theme = use_theme();
    let text = create_memo(move || match hidden.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let toggle = move || state.set_hidden(id, !hidden.get_untracked());
    let select = move || state.select(id, false);
    let dropped = move |(Layer(moved), point): (Layer, DragPoint)| {
        let in_front = point.fraction.y < 0.5;
        state.move_layer(moved, id, in_front);
    };
    let ghost = move |_: Layer| {
        view! {
            <LayerGhost label={label} />
        }
    };
    view! {
        <DropTarget on_drop={dropped}>
            {move |handle: DropHandle| {
                let fill = create_memo(move || match selected.get() {
                    true => theme.accent_soft.get(),
                    false => Color32::TRANSPARENT,
                });
                view! {
                    <Frame
                        fill={ColorRole::custom(fill)}
                        radius={Radius::custom(ROW_RADIUS)}
                        border={create_memo(move || handle.over.get().then_some(Border::new(ColorRole::Accent).width(1.0)))}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                            <IconButton
                                glyph={glyph}
                                label={toggle_label}
                                size=IconButtonSize::Compact
                                variant=ButtonVariant::Ghost
                                capture_presses=true
                                @test_id={format!("infinite-canvas.layer.{id}.visibility")}
                                on_click={toggle}
                            />
                            <Draggable
                                @sizing=ItemSize::Percent(100.0)
                                payload={Some(Layer(id))}
                                cursor=CursorIcon::PointingHand
                                preview={ghost}
                                on_click={select}
                                @test_id={format!("infinite-canvas.layer.{id}")}
                            >
                                {move |_: DragHandle| view! {
                                    <Frame padding={Sides::vertical(ROW_PADDING)}>
                                        <Text string={label} color={text} />
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
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            radius={Radius::custom(ROW_RADIUS)}
            padding={Sides::symmetric(8.0, ROW_PADDING)}
        >
            <Text string={label} />
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
