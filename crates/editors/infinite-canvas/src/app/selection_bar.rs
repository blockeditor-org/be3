use std::rc::Rc;

use block_editor_beui::be_block::canvas::{CanvasColor, CanvasEntityKind, CanvasLayerMove};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node as AccessNode, Role};
use block_editor_beui::beui::icons::{
    ICON_DELETE, ICON_FLIP_TO_FRONT, ICON_OPEN_IN_NEW, ICON_TUNE,
};
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, List, Memo, Show, clone, component, create_memo,
    view,
};
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::{IconButton, use_theme};
use block_editor_beui::beui::unstyled::{self, ButtonHandle};
use block_editor_beui::{narrow_chrome, sheet_control};
use uuid::Uuid;

use super::paint::resolve_color;
use super::sidebar::{PRESETS, set_foreground, styled_entities};
use super::state::{CanvasCommand, CanvasState, CommonValue, common_value};

const RADIUS: u8 = 12;
const SWATCH: f32 = 22.0;
const SWATCH_RING: f32 = 3.0;

#[component]
pub(crate) fn SelectionTools(state: Rc<CanvasState>) -> NodeId {
    let narrow = narrow_chrome();
    let colored = create_memo(clone!(state -> move || !styled_entities(&state).is_empty()));
    let value = create_memo(clone!(state -> move || {
        common_value(
            styled_entities(&state)
                .iter()
                .map(|entity| entity.style.foreground),
        )
    }));
    let locked = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| !entity.locked)
    }));
    let embedded = create_memo(clone!(state -> move || embedded_block(&state).is_some()));
    let picking = Rc::clone(&state);
    let front =
        clone!(state -> move || state.run(CanvasCommand::Reorder(CanvasLayerMove::BringToFront)));
    let delete = clone!(state -> move || state.run(CanvasCommand::Delete));
    let open = clone!(state -> move || {
        if let Some(block) = embedded_block(&state) {
            state.open_referenced_block(block);
        }
    });
    let sheet = sheet_control();
    let tune = move || {
        if let Some(sheet) = &sheet {
            sheet.set(true);
        }
    };
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=RADIUS
            padding_horizontal=6.0
            padding_vertical=6.0
            @test_id={"infinite-canvas.selection-bar"}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Show condition={colored}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
                        <ForEach keys={(0..PRESETS.len()).collect::<Vec<usize>>()}>
                            {move |index: usize| {
                                let (name, color) = PRESETS[index];
                                let state = Rc::clone(&picking);
                                let value = value.clone();
                                view! {
                                    <Swatch
                                        kind="color"
                                        name
                                        color
                                        value
                                        on_pick={move |color| set_foreground(&state, color)}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                </Show>
                <IconButton
                    glyph={ICON_FLIP_TO_FRONT.to_owned()}
                    label="Bring to front"
                    disabled={locked.clone()}
                    @test_id={"infinite-canvas.front"}
                    on_click={front}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Delete"
                    disabled={locked}
                    @test_id={"infinite-canvas.delete"}
                    on_click={delete}
                />
                <Show condition={embedded}>
                    <IconButton
                        glyph={ICON_OPEN_IN_NEW.to_owned()}
                        label="Open"
                        @test_id={"infinite-canvas.open"}
                        on_click={open}
                    />
                </Show>
                <Show condition={narrow}>
                    <IconButton
                        glyph={ICON_TUNE.to_owned()}
                        label="Inspector"
                        @test_id={"infinite-canvas.inspect"}
                        on_click={tune}
                    />
                </Show>
            </List>
        </Frame>
    }
}

fn embedded_block(state: &CanvasState) -> Option<Uuid> {
    let selected = state.selected_entities();
    let [entity] = selected.as_slice() else {
        return None;
    };
    match entity.kind {
        CanvasEntityKind::Block { block_id } | CanvasEntityKind::DirectEditor { block_id, .. } => {
            Some(block_id)
        }
        _ => None,
    }
}

#[component]
pub(crate) fn Swatch(
    kind: &'static str,
    name: &'static str,
    color: CanvasColor,
    value: Memo<CommonValue<CanvasColor>>,
    on_pick: Callback<CanvasColor>,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || resolve_color(color, theme.text.get())));
    let chosen = create_memo(move || value.get() == CommonValue::Uniform(color));
    let mut described = AccessNode::new(Role::Button);
    described.set_label(format!("{name} {kind}"));
    view! {
        <unstyled::Button
            @test_id={format!("infinite-canvas.{kind}.{name}")}
            accessibility={described}
            on_click={move || on_pick.call(color)}
            content={move |handle: ButtonHandle| {
                let ringed = create_memo(clone!(chosen -> move || {
                    chosen.get() || handle.focused.get() || handle.hovered.get()
                }));
                let ring = create_memo(clone!(theme chosen -> move || match chosen.get() {
                    true => theme.accent.get(),
                    false => theme.border.get(),
                }));
                view! {
                    <Frame
                        padding_horizontal=SWATCH_RING
                        padding_vertical=SWATCH_RING
                        radius=16
                        outline={ring}
                        outline_width=2.0
                        outline_visible={ringed}
                    >
                        <Frame
                            width=SWATCH
                            height=SWATCH
                            radius=11
                            color={fill.clone()}
                            outline={theme.border.clone()}
                            outline_width=BORDER_WIDTH
                            outline_visible=true
                        />
                    </Frame>
                }
            }}
        />
    }
}
