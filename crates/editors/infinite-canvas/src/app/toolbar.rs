use block_editor_beui::be_block::canvas::CanvasProfile;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_REDO, ICON_UNDO};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{Edge, Floating};
use block_editor_beui::{BottomDock, node_anchor, sheet_open};

use super::actions::CanvasActions;
use super::selection_bar::SelectionTools;
use super::state::{CanvasState, Tool};

const TOOLS: [(Tool, &str); 10] = [
    (Tool::Select, "Select"),
    (Tool::Pen, "Pen"),
    (Tool::Rectangle, "Rectangle"),
    (Tool::Ellipse, "Ellipse"),
    (Tool::Diamond, "Diamond"),
    (Tool::Arrow, "Arrow"),
    (Tool::Line, "Line"),
    (Tool::Text, "Text"),
    (Tool::Frame, "Frame"),
    (Tool::Eraser, "Eraser"),
];
const PILL_MARGIN: f32 = 12.0;
const PILL_RADIUS: f32 = 16.0;
const PILL_PADDING: f32 = 4.0;
const DIVIDER_HEIGHT: f32 = 24.0;

#[component]
pub(crate) fn ToolDock(state: CanvasState, anchor: NodeRef, shown: Prop<bool>) -> NodeId {
    let previewing = state.previewing();
    let open = create_memo(move || shown.get() && !previewing);
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let tools = actions.clone();
    let chosen = create_memo(move || {
        !state.selection.get().is_empty()
            || (state.profile.get() == CanvasProfile::Prototyping && state.tool.get().styles())
    });
    let tool_keys = create_memo(move || {
        let profile = state.profile.get();
        (0..TOOLS.len())
            .filter(|index| TOOLS[*index].0.belongs_to(profile))
            .collect::<Vec<usize>>()
    });
    let selection = move || {
        view! {
            <List align=Align::Center spacing=0.0>
                <SelectionTools state />
                <Frame height=PILL_MARGIN />
            </List>
        }
    };
    let history = state.editor().history(state.editor().block_id());
    let stuck = create_memo(move || !history.get().can_undo);
    let redo_stuck = create_memo(move || !history.get().can_redo);
    let undo = move || {
        let editor = state.editor();
        editor.host().undo(editor.block_id());
    };
    let block_button = NodeRef::new();
    let add_block = move || match node_anchor(&block_button) {
        Some(button) => state.open_block_picker_at(button),
        None => state.open_block_picker(None),
    };
    let redo = move || {
        let editor = state.editor();
        editor.host().redo(editor.block_id());
    };
    view! {
        <BottomDock
            anchor={anchor}
            open={open}
            name="infinite-canvas.dock"
            above={view! {
                <List spacing=0.0>
                    <Show condition={chosen} then={selection} />
                </List>
            }}
        >
            <ToggleButton
                icon_only=true
                action={actions.tool_lock.clone()}
                @test_id={"infinite-canvas.dock.tool-lock"}
            />
            <ForEach keys={tool_keys}>|index: usize| {
                let actions = tools.clone();
                view! {
                    <ToolChoice state actions index />
                }
            }</ForEach>
            <IconButton
                @node_ref={&block_button}
                label="Block"
                glyph={actions.add_block.glyph().to_owned()}
                disabled={actions.add_block.disabled()}
                @test_id={"infinite-canvas.dock.block"}
                on_click={add_block}
            />
            <Frame width=BORDER_WIDTH height=DIVIDER_HEIGHT fill=ColorRole::Border />
            <IconButton
                glyph={ICON_UNDO.to_owned()}
                label="Undo"
                disabled={stuck}
                @test_id={"infinite-canvas.dock.undo"}
                on_click={undo}
            />
            <IconButton
                glyph={ICON_REDO.to_owned()}
                label="Redo"
                disabled={redo_stuck}
                @test_id={"infinite-canvas.dock.redo"}
                on_click={redo}
            />
            <Frame width=BORDER_WIDTH height=DIVIDER_HEIGHT fill=ColorRole::Border />
            <ToggleButton
                icon_only=true
                action={actions.ignore_touch.clone()}
                @test_id={"infinite-canvas.dock.ignore-touch"}
            />
        </BottomDock>
    }
}

#[component]
pub(crate) fn ZoomPill(state: CanvasState, anchor: NodeRef, shown: Prop<bool>) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let sheet = sheet_open();
    let previewing = state.previewing();
    let open = create_memo(move || shown.get() && !sheet.get() && !previewing);
    let scale = state.editor().scale();
    let readout = create_memo(move || format!("{:.0}%", scale.get() * 100.0));
    let reset = actions.zoom_reset.clone();
    view! {
        <Floating anchor={anchor} edge=Edge::TopEnd open={open}>
            <Frame padding=PILL_MARGIN>
                <Frame
                    fill=ColorRole::SurfaceRaised
                    border={Border::DEFAULT}
                    radius={Radius::custom(PILL_RADIUS)}
                    padding=PILL_PADDING
                    @test_id={"infinite-canvas.zoom-pill"}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
                        <IconButton
                            @test_id={"infinite-canvas.pill.zoom-out"}
                            action={actions.zoom_out}
                        />
                        <Button
                            label={readout}
                            variant=ButtonVariant::Ghost
                            @test_id={"infinite-canvas.pill.zoom"}
                            on_click={move || {
                                reset.run();
                            }}
                        />
                        <IconButton
                            @test_id={"infinite-canvas.pill.zoom-in"}
                            action={actions.zoom_in}
                        />
                        <IconButton
                            @test_id={"infinite-canvas.pill.fit"}
                            action={actions.fit_all}
                        />
                    </List>
                </Frame>
            </Frame>
        </Floating>
    }
}

#[component]
fn ToolChoice(state: CanvasState, actions: CanvasActions, index: usize) -> NodeId {
    let (tool, name) = TOOLS[index];
    let (label, glyph, action) = match tool {
        Tool::Select => {
            let mode = create_memo(move || state.select_tool());
            (
                Prop::dynamic(move || match mode.get() {
                    Tool::Lasso => "Lasso select".to_owned(),
                    _ => name.to_owned(),
                }),
                Prop::dynamic(move || mode.get().glyph().to_owned()),
                actions.select_mode,
            )
        }
        _ => (
            Prop::Static(name.to_owned()),
            Prop::Static(tool.glyph().to_owned()),
            actions.tool(tool),
        ),
    };
    view! {
        <ToggleButton
            label={label}
            glyph={glyph}
            icon_only=true
            action={action}
            @test_id={format!("infinite-canvas.dock.tool.{name}")}
        />
    }
}

#[component]
pub(crate) fn ImportError(state: CanvasState) -> NodeId {
    let failed = create_memo(move || state.import_error.get().is_some());
    let message = create_memo(move || state.import_error.get().unwrap_or_default());
    let dismiss = move || state.dismiss_import_error();
    view! {
        <List spacing=0.0>
            <Show condition={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text color=TextColor::Danger>{message}</Text>
                    <Button
                        label="Dismiss"
                        variant=ButtonVariant::Secondary
                        @test_id={"infinite-canvas.dismiss-error"}
                        on_click={dismiss}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </Show>
        </List>
    }
}
