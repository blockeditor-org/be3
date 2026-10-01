use std::rc::Rc;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CROP_FREE, ICON_DIAGONAL_LINE, ICON_DRAW, ICON_RECTANGLE, ICON_SELECT, ICON_TEXT_FIELDS,
    ICON_UNDO,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, NodeRef, Prop, Show, Spacer, clone,
    component, create_memo, use_context, view,
};
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, IconButton, ToggleButton, use_theme,
};
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

const DOCK_TOOLS: [usize; 4] = [0, 5, 3, 4];
const PILL_MARGIN: f32 = 12.0;
const PILL_RADIUS: u8 = 16;
const PILL_PADDING: f32 = 4.0;
const DIVIDER_HEIGHT: f32 = 24.0;

#[component]
pub(crate) fn ToolDock(state: Rc<CanvasState>, anchor: NodeRef, shown: Prop<bool>) -> NodeId {
    let previewing = state.previewing();
    let open = create_memo(move || shown.get() && !previewing);
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let tools = actions.clone();
    let chosen = create_memo(clone!(state -> move || !state.selection.get().is_empty()));
    let selection = clone!(state -> move || {
        let state = Rc::clone(&state);
        view! {
            <List align=Align::Center spacing=0.0>
                <SelectionTools state />
                <Frame height=PILL_MARGIN />
            </List>
        }
    });
    let history = state.editor().history(state.editor().block_id());
    let stuck = create_memo(move || !history.get().can_undo);
    let undo = clone!(state -> move || {
        let editor = state.editor();
        editor.host().undo(editor.block_id());
    });
    let theme = use_theme();
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
                {move |index: usize| {
                    let actions = tools.clone();
                    view! {
                        <ToolChoice actions index />
                    }
                }}
            </ForEach>
            <IconButton
                label="Block"
                @test_id={"infinite-canvas.dock.block"}
                action={actions.add_block}
            />
            <Frame width=BORDER_WIDTH height=DIVIDER_HEIGHT color={theme.border.clone()} />
            <IconButton
                glyph={ICON_UNDO.to_owned()}
                label="Undo"
                disabled={stuck}
                @test_id={"infinite-canvas.dock.undo"}
                on_click={undo}
            />
        </BottomDock>
    }
}

#[component]
pub(crate) fn ZoomPill(state: Rc<CanvasState>, anchor: NodeRef, shown: Prop<bool>) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let sheet = sheet_open();
    let previewing = state.previewing();
    let open = create_memo(move || shown.get() && !sheet.get() && !previewing);
    let scale = state.editor().scale();
    let readout = create_memo(clone!(scale -> move || format!("{:.0}%", scale.get() * 100.0)));
    let reset = actions.zoom_reset.clone();
    let theme = use_theme();
    view! {
        <Floating anchor={anchor} edge=Edge::TopEnd open={open}>
            <Frame padding_vertical=PILL_MARGIN padding_horizontal=PILL_MARGIN>
                <Frame
                    color={theme.surface_raised.clone()}
                    outline={theme.border.clone()}
                    outline_width=BORDER_WIDTH
                    outline_visible=true
                    radius=PILL_RADIUS
                    padding_horizontal=PILL_PADDING
                    padding_vertical=PILL_PADDING
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
pub(crate) fn ImportError(state: Rc<CanvasState>) -> NodeId {
    let failed = create_memo(clone!(state -> move || state.import_error.get().is_some()));
    let message =
        create_memo(clone!(state -> move || state.import_error.get().unwrap_or_default()));
    let dismiss = clone!(state -> move || state.dismiss_import_error());
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <Show condition={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Body content={message} color={theme.danger.clone()} />
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
