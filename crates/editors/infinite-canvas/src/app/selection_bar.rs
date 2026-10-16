use super::actions::{CanvasActions, embedded_block};
use super::paint::resolve_color;
use super::sidebar::{PRESETS, set_foreground, styled_entities};
use super::state::{CanvasState, CommonValue, common_value};
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node as AccessNode, Role};
use block_editor_beui::beui::icons::ICON_TUNE;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, ButtonHandle};
use block_editor_beui::{narrow_chrome, sheet_control};

const RADIUS: f32 = 12.0;
const SWATCH: f32 = 22.0;
const SWATCH_RING: f32 = 3.0;

#[component]
pub(crate) fn SelectionTools(state: CanvasState) -> NodeId {
    let narrow = narrow_chrome();
    let colored = create_memo(move || !styled_entities(state).is_empty());
    let value = create_memo(move || {
        common_value(
            styled_entities(state)
                .iter()
                .map(|entity| entity.style.foreground),
        )
    });
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let open = actions.open.clone();
    let embedded = create_memo(move || embedded_block(&state).is_some());
    let sheet = sheet_control();
    let tune = move || {
        if let Some(sheet) = &sheet {
            sheet.set(true);
        }
    };
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius={Radius::custom(RADIUS)}
            padding=6.0
            @test_id={"infinite-canvas.selection-bar"}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Show condition={colored}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
                            <ForEach keys={(0..PRESETS.len()).collect::<Vec<usize>>()}>
                                |index: usize| {
                                    let (name, color) = PRESETS[index];
                                    let value = value;
                                    view! {
                                        <Swatch
                                            kind="color"
                                            name
                                            color
                                            value
                                            on_pick={move |color| set_foreground(state, color)}
                                        />
                                    }
                                }
                            </ForEach>
                        </List>
                </Show>
                <IconButton @test_id={"infinite-canvas.front"} action={actions.front} />
                <IconButton @test_id={"infinite-canvas.delete"} action={actions.delete} />
                <Show condition={embedded}>
                    || { let open = open.clone(); view! {
                        <IconButton label="Open" @test_id={"infinite-canvas.open"} action={open} />
                    } }
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

#[component]
pub(crate) fn Swatch(
    kind: &'static str,
    name: &'static str,
    color: CanvasColor,
    value: Memo<CommonValue<CanvasColor>>,
    on_pick: Callback<CanvasColor>,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || resolve_color(color, theme.text.get()));
    let chosen = create_memo(move || value.get() == CommonValue::Uniform(color));
    let mut described = AccessNode::new(Role::Button);
    described.set_label(format!("{name} {kind}"));
    view! {
        <unstyled::Button
            @test_id={format!("infinite-canvas.{kind}.{name}")}
            accessibility={described}
            on_click={move || on_pick.call(color)}
            content={move |handle: ButtonHandle| {
                let ringed = create_memo(move || {
                    chosen.get() || handle.focused.get() || handle.hovered.get()
                });
                let ring = create_memo(move || match chosen.get() {
                    true => theme.accent.get(),
                    false => theme.border.get(),
                });
                view! {
                    <Frame
                        padding=SWATCH_RING
                        radius={Radius::custom(16.0)}
                        border={create_memo(move || ringed.get().then(|| Border::new(ColorRole::Custom(ring.get())).width(2.0)))}
                    >
                        <Frame
                            width=SWATCH
                            height=SWATCH
                            radius={Radius::custom(11.0)}
                            fill={ColorRole::custom(fill)}
                            border={Border::DEFAULT}
                        />
                    </Frame>
                }
            }}
        />
    }
}
