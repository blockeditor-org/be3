use std::rc::Rc;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CONTENT_COPY, ICON_CONTENT_CUT, ICON_CONTENT_PASTE, ICON_CROP_FREE, ICON_DATA_OBJECT,
    ICON_DELETE, ICON_DESELECT, ICON_DIAGONAL_LINE, ICON_DRAW, ICON_FILTER_CENTER_FOCUS,
    ICON_FIT_SCREEN, ICON_GROUP_WORK, ICON_LIBRARY_ADD, ICON_LOCK, ICON_LOCK_OPEN, ICON_RECTANGLE,
    ICON_SELECT, ICON_SELECT_ALL, ICON_TEXT_FIELDS, ICON_UNDO, ICON_WORKSPACES, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef, Prop, Show, Spacer, clone,
    component, create_memo, view,
};
use block_editor_beui::beui::styled::theme::BORDER_WIDTH;
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, IconButton, ToggleButton, use_theme,
};
use block_editor_beui::beui::unstyled::{Edge, Floating};
use block_editor_beui::{BottomDock, bar_item, sheet_open};

use super::selection_bar::SelectionTools;
use super::state::{CanvasCommand, CanvasState, Tool, ZOOM_STEP};

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
    let tools = Rc::clone(&state);
    let blocks = Rc::clone(&state);
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
                    let state = Rc::clone(&tools);
                    view! {
                        <ToolChoice state index />
                    }
                }}
            </ForEach>
            <IconButton
                glyph={ICON_DATA_OBJECT.to_owned()}
                label="Block"
                @test_id={"infinite-canvas.dock.block"}
                on_click={move || blocks.open_block_picker(None)}
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
    let sheet = sheet_open();
    let previewing = state.previewing();
    let open = create_memo(move || shown.get() && !sheet.get() && !previewing);
    let scale = state.editor().scale();
    let readout = create_memo(clone!(scale -> move || format!("{:.0}%", scale.get() * 100.0)));
    let out = clone!(state -> move || state.editor().zoom(1.0 / ZOOM_STEP));
    let reset = clone!(state scale -> move || {
        state.editor().zoom(1.0 / scale.get_untracked().max(f32::EPSILON))
    });
    let inward = clone!(state -> move || state.editor().zoom(ZOOM_STEP));
    let fit = clone!(state -> move || state.editor().fit());
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
                            glyph={ICON_ZOOM_OUT.to_owned()}
                            label="Zoom out"
                            @test_id={"infinite-canvas.pill.zoom-out"}
                            on_click={out}
                        />
                        <Button
                            label={readout}
                            variant=ButtonVariant::Ghost
                            @test_id={"infinite-canvas.pill.zoom"}
                            on_click={reset}
                        />
                        <IconButton
                            glyph={ICON_ZOOM_IN.to_owned()}
                            label="Zoom in"
                            @test_id={"infinite-canvas.pill.zoom-in"}
                            on_click={inward}
                        />
                        <IconButton
                            glyph={ICON_FIT_SCREEN.to_owned()}
                            label="Fit all"
                            @test_id={"infinite-canvas.pill.fit"}
                            on_click={fit}
                        />
                    </List>
                </Frame>
            </Frame>
        </Floating>
    }
}

pub(crate) fn menu_items(state: &Rc<CanvasState>) {
    let empty = create_memo(clone!(state -> move || state.selection.get().is_empty()));
    let never = create_memo(|| false);
    let grouped = create_memo(clone!(state -> move || !state.selection_can_group()));
    let ungrouped = create_memo(clone!(state -> move || {
        !state
            .selected_entities()
            .iter()
            .any(|entity| entity.group_id.is_some())
    }));
    let lockable = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| !entity.locked)
    }));
    let unlockable = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| entity.locked)
    }));
    let command = |label: &str, glyph: &str, disabled: &Memo<bool>, command: CanvasCommand| {
        let state = Rc::clone(state);
        bar_item(label, glyph, disabled.clone(), move || state.run(command));
    };
    command("Cut", ICON_CONTENT_CUT, &empty, CanvasCommand::Cut);
    command("Copy", ICON_CONTENT_COPY, &empty, CanvasCommand::Copy);
    command("Paste", ICON_CONTENT_PASTE, &never, CanvasCommand::Paste);
    command(
        "Duplicate",
        ICON_LIBRARY_ADD,
        &empty,
        CanvasCommand::Duplicate,
    );
    command("Delete", ICON_DELETE, &empty, CanvasCommand::Delete);
    command("Group", ICON_GROUP_WORK, &grouped, CanvasCommand::Group);
    command(
        "Ungroup",
        ICON_WORKSPACES,
        &ungrouped,
        CanvasCommand::Ungroup,
    );
    command("Lock", ICON_LOCK, &lockable, CanvasCommand::Lock);
    command("Unlock", ICON_LOCK_OPEN, &unlockable, CanvasCommand::Unlock);
    command(
        "Select all",
        ICON_SELECT_ALL,
        &never,
        CanvasCommand::SelectAll,
    );
    command(
        "Invert selection",
        ICON_DESELECT,
        &never,
        CanvasCommand::InvertSelection,
    );
    bar_item(
        "Fit selection",
        ICON_FILTER_CENTER_FOCUS,
        empty,
        clone!(state -> move || state.request_fit_selection()),
    );
    bar_item(
        "Fit the first artboard",
        ICON_FIT_SCREEN,
        create_memo(clone!(state -> move || state.first_artboard().is_none())),
        clone!(state -> move || state.request_fit_artboard()),
    );
    bar_item(
        "Draw an artboard",
        ICON_CROP_FREE,
        never.clone(),
        clone!(state -> move || state.set_tool(Tool::Artboard)),
    );
    bar_item(
        "Draw a line",
        ICON_DIAGONAL_LINE,
        never,
        clone!(state -> move || state.set_tool(Tool::Line)),
    );
}

#[component]
fn ToolChoice(state: Rc<CanvasState>, index: usize) -> NodeId {
    let (tool, glyph, label) = TOOLS[index];
    let pressed = create_memo(clone!(state -> move || state.tool.get() == tool));
    let choose = clone!(state -> move |_: bool| state.set_tool(tool));
    view! {
        <ToggleButton
            label={label}
            glyph={glyph.to_owned()}
            icon_only=true
            pressed={pressed}
            @test_id={format!("infinite-canvas.dock.tool.{label}")}
            on_change={choose}
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
                {move || clone!(dismiss message theme -> view! {
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
                })}
            </Show>
        </List>
    }
}
