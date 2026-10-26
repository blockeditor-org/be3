use super::actions::{CanvasActions, embedded_block};
use super::paint::resolve_color;
use super::sidebar::{set_foreground, styled_entities};
use super::state::{CanvasState, CommonValue, common_value};
use super::style::{STROKES, StylePopover};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node as AccessNode, Role};
use block_editor_beui::beui::icons::ICON_SLIDERS_HORIZONTAL;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, ButtonHandle};
use block_editor_beui::{narrow_chrome, sheet_control};
use infinite_canvas_types::{CanvasColor, CanvasProfile};

const RADIUS: f32 = 12.0;
const SWATCH: f32 = 22.0;
const SWATCH_RING: f32 = 3.0;

#[component]
pub(crate) fn SelectionTools(state: CanvasState) -> NodeId {
    let narrow = narrow_chrome();
    let prototyping = create_memo(move || state.profile.get() == CanvasProfile::Prototyping);
    let vector = create_memo(move || !prototyping.get());
    let chosen = create_memo(move || !state.selection.get().is_empty());
    let inspectable = create_memo(move || vector.get() && narrow.get());
    let colored = create_memo(move || vector.get() && !styled_entities(state).is_empty());
    let value = create_memo(move || {
        common_value(
            styled_entities(state)
                .iter()
                .map(|entity| entity.style.stroke_color),
        )
    });
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let open = actions.open.clone();
    let bind_lines = actions.bind_lines.clone();
    let lining = create_memo(move || state.tool.get().draws_a_line());
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
                <Show condition={prototyping}>
                    <StylePopover state={state} />
                </Show>
                <Show condition={lining}>|| {
                    let bind_lines = bind_lines.clone(); view! {
                        <ToggleButton
                            icon_only=true
                            action={bind_lines}
                            @test_id={"infinite-canvas.bind-lines"}
                        />
                    }
                }</Show>
                <Show condition={colored}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
                        <ForEach keys={(0..STROKES.len()).collect::<Vec<usize>>()}>|index: usize| {
                            let (name, color) = STROKES[index];
                            view! {
                                <Swatch
                                    kind="color"
                                    name
                                    color
                                    value
                                    on_pick={move |color| set_foreground(state, color)}
                                />
                            }
                        }</ForEach>
                    </List>
                </Show>
                <Show condition={chosen}>|| {
                    let actions = actions.clone(); view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                            <Button
                                icon_only=true
                                @test_id={"infinite-canvas.front"}
                                action={actions.front}
                            />
                            <Button
                                icon_only=true
                                @test_id={"infinite-canvas.delete"}
                                action={actions.delete}
                            />
                        </List>
                    }
                }</Show>
                <Show condition={embedded}>|| {
                    let open = open.clone(); view! {
                        <Button
                            icon_only=true
                            label="Open"
                            @test_id={"infinite-canvas.open"}
                            action={open}
                        />
                    }
                }</Show>
                <Show condition={inspectable}>
                    <Button
                        icon_only=true
                        glyph=ICON_SLIDERS_HORIZONTAL
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
        >|handle: ButtonHandle| {
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
        }</unstyled::Button>
    }
}
