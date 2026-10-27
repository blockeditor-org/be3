use block_editor_beui::beui::icons::{ICON_EYE, ICON_EYE_OFF};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{CursorIcon, NodeId};
use infinite_canvas_types::{CanvasEntity, CanvasShape};
use uuid::Uuid;

use crate::geometry::entity_kind_label;

use super::state::CanvasState;

const ROW_PADDING: Space = Space::Xs;
const ROW_RADIUS: Radius = Radius::Sm;

#[component]
pub(crate) fn LayersSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let entities = state.entities.ids();
    let ids =
        create_memo(move || entities.with(|ids| ids.iter().rev().copied().collect::<Vec<Uuid>>()));
    let empty = create_memo(move || ids.with(Vec::is_empty));
    let read_only = state.editor().read_only();
    let moved = move |moved: ReorderMove<Uuid, Uuid>| state.move_layer(moved.payload, moved.after);
    let text = move |id: Uuid| {
        state
            .entities
            .get_untracked(id)
            .map(|entity| layer_label(&state, &entity))
            .unwrap_or_default()
    };
    let ghost = move |id: Uuid| {
        view! {
            <LayerGhost label={text(id)} />
        }
    };
    view! {
        <Accordion title="Layers" open={open}>
            <List spacing=Space::Xs2>
                <Show condition={empty}>
                    <Text style=TextStyle::CAPTION>"Nothing on the canvas yet."</Text>
                </Show>
                <Reorder
                    keys={ids}
                    payload={Func::new(Some)}
                    disabled={read_only}
                    text={text}
                    preview={ghost}
                    on_move={moved}
                >|reorder: ReorderHandle<Uuid, Uuid>|
                    <List spacing=Space::Xs2 @test_id={"infinite-canvas.layers"}>
                        <ForEach keys={reorder.order}>|id: Uuid|
                            <LayerRow state reorder id />
                        </ForEach>
                    </List>
                </Reorder>
            </List>
        </Accordion>
    }
}

#[component]
fn LayerRow(state: CanvasState, reorder: ReorderHandle<Uuid, Uuid>, id: Uuid) -> NodeId {
    let entity = state.entities.watch(id).stored();
    let label = create_memo(move || {
        entity
            .get()
            .map(|entity| layer_label(&state, &entity))
            .unwrap_or_default()
    });
    let selected = create_memo(move || state.selection.get().contains(&id));
    let hidden = create_memo(move || entity.get().is_some_and(|entity| entity.hidden));
    let glyph = create_memo(move || match hidden.get() {
        true => ICON_EYE_OFF,
        false => ICON_EYE,
    });
    let toggle_label = create_memo(move || match hidden.get() {
        true => "Show".to_owned(),
        false => "Hide".to_owned(),
    });
    let text = create_memo(move || match hidden.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let fill = create_memo(move || match selected.get() {
        true => ColorRole::AccentSoft,
        false => ColorRole::None,
    });
    let toggle = move || state.set_hidden(id, !hidden.get_untracked());
    let select = move || state.select(id, false);
    view! {
        <ReorderItem
            reorder
            item=id
            radius=ROW_RADIUS
            cursor=CursorIcon::PointingHand
            on_click={select}
        >|_: ReorderItemHandle|
            <Frame fill={fill} radius=ROW_RADIUS>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <Button
                        icon_only=true
                        glyph={glyph}
                        label={toggle_label}
                        size=ButtonSize::Small
                        variant=ButtonVariant::Ghost
                        capture_presses=true
                        @test_id={format!("infinite-canvas.layer.{id}.visibility")}
                        on_click={toggle}
                    />
                    <Frame
                        @sizing=ItemSize::Percent(100.0)
                        padding={Padding::vertical(ROW_PADDING)}
                        @test_id={format!("infinite-canvas.layer.{id}")}
                    >
                        <Text color={text}>{label}</Text>
                    </Frame>
                </List>
            </Frame>
        </ReorderItem>
    }
}

#[component]
fn LayerGhost(label: String) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            radius=ROW_RADIUS
            padding={Padding::symmetric(Space::Md, ROW_PADDING)}
        >
            <Text>{label}</Text>
        </Frame>
    }
}

fn layer_label(state: &CanvasState, entity: &CanvasEntity) -> String {
    let text = entity.text.to_str_lossy();
    let written = text.lines().next().filter(|line| !line.trim().is_empty());
    match (entity.shape, written, entity.block) {
        (_, Some(line), _) => line.to_owned(),
        (CanvasShape::Text, None, _) => entity.placeholder.clone(),
        (shape, None, Some(block)) if shape.embeds_block() => state
            .label_of(block)
            .map(|label| label.name)
            .unwrap_or_else(|| entity_kind_label(entity).to_owned()),
        _ => entity_kind_label(entity).to_owned(),
    }
}
