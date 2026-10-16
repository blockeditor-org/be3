use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CROP_FREE, ICON_DIAGONAL_LINE, ICON_DRAW, ICON_RECTANGLE, ICON_REDO, ICON_SELECT,
    ICON_TEXT_FIELDS, ICON_UNDO,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{Edge, Floating};
use block_editor_beui::{BottomDock, sheet_open};

use super::actions::CanvasActions;
use super::selection_bar::SelectionTools;
use super::state::{CanvasState, Tool};

const TOOLS: [(Tool, &str, &str); 6] = [
    (Tool::Select, ICON_SELECT, "Select"),
    (Tool::Artboard, ICON_CROP_FREE, "Artboard"),
    (Tool::Line, ICON_DIAGONAL_LINE, "Line"),
    (Tool::Rectangle, ICON_RECTANGLE, "Rectangle"),
    (Tool::Text, ICON_TEXT_FIELDS, "Text"),
    (Tool::Pen, ICON_DRAW, "Pen"),
];

const DOCK_TOOLS: [usize; 6] = [0, 5, 2, 3, 4, 1];
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
    let chosen = create_memo(move || !state.selection.get().is_empty());
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
            <ForEach keys={DOCK_TOOLS.to_vec()}>
                |index: usize| {
                    let actions = tools.clone();
                    view! {
                        <ToolChoice actions index />
                    }
                }
            </ForEach>
            <IconButton
                label="Block"
                @test_id={"infinite-canvas.dock.block"}
                action={actions.add_block}
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
fn ToolChoice(actions: CanvasActions, index: usize) -> NodeId {
    let (tool, glyph, label) = TOOLS[index];
    view! {
        <ToggleButton
            label={label}
            glyph={glyph.to_owned()}
            icon_only=true
            action={actions.tool(tool)}
            @test_id={format!("infinite-canvas.dock.tool.{label}")}
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
                        <Text string={message} color=TextColor::Danger />
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
