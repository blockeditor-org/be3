use std::rc::Rc;

use block_editor_beui::Toolbar;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_DATA_OBJECT, ICON_DIAGONAL_LINE, ICON_DRAW, ICON_KEYBOARD_ARROW_DOWN, ICON_RECTANGLE,
    ICON_SELECT, ICON_TEXT_FIELDS, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Prop, Show, Spacer, clone, component, create_memo,
    view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, IconButton, MenuButton, ToggleButton, use_theme,
};
use block_editor_beui::beui::unstyled::MenuItem;

use super::state::{CanvasCommand, CanvasState, Tool, ZOOM_STEP};

const TOOLS: [(Tool, &str, &str); 5] = [
    (Tool::Select, ICON_SELECT, "Select"),
    (Tool::Line, ICON_DIAGONAL_LINE, "Line"),
    (Tool::Rectangle, ICON_RECTANGLE, "Rectangle"),
    (Tool::Text, ICON_TEXT_FIELDS, "Text"),
    (Tool::Pen, ICON_DRAW, "Pen"),
];

const ACTIONS: [(&str, CanvasCommand); 5] = [
    ("Cut", CanvasCommand::Cut),
    ("Copy", CanvasCommand::Copy),
    ("Paste", CanvasCommand::Paste),
    ("Duplicate", CanvasCommand::Duplicate),
    ("Delete", CanvasCommand::Delete),
];

const ZOOM_PRESETS: [f32; 4] = [0.25, 0.5, 1.0, 2.0];

#[component]
pub(crate) fn CanvasToolbar(state: Rc<CanvasState>, shown: Prop<bool>) -> NodeId {
    let tools = Rc::clone(&state);
    let blocks = Rc::clone(&state);
    let actions = Rc::clone(&state);
    let zoom = Rc::clone(&state);
    let errors = Rc::clone(&state);
    view! {
        <Toolbar shown={shown}>
            <List @sizing=ItemSize::Percent(100.0) spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0 wrap=true>
                    <ForEach keys={(0..TOOLS.len()).collect::<Vec<usize>>()}>
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
                        @test_id={"infinite-canvas.add-block"}
                        on_click={move || blocks.open_block_picker(None)}
                    />
                    <ActionsMenu state={actions} />
                    <ZoomControls state={zoom} />
                </List>
                <ImportError state={errors} />
            </List>
        </Toolbar>
    }
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
            pressed={pressed}
            @test_id={format!("infinite-canvas.tool.{label}")}
            on_change={choose}
        />
    }
}

#[component]
fn ActionsMenu(state: Rc<CanvasState>) -> NodeId {
    let selected = create_memo(clone!(state -> move || state.selection.get().is_empty()));
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
    let chosen = clone!(state -> move |path: Vec<usize>| {
        let Some(index) = path.first().copied() else {
            return;
        };
        match index {
            0..=4 => state.run(ACTIONS[index].1),
            5 => state.run(CanvasCommand::Group),
            6 => state.run(CanvasCommand::Ungroup),
            7 => state.run(CanvasCommand::Lock),
            8 => state.run(CanvasCommand::Unlock),
            9 => state.run(CanvasCommand::SelectAll),
            10 => state.run(CanvasCommand::InvertSelection),
            _ => state.request_fit_selection(),
        }
    });
    let paste = create_memo(|| false);
    let items = view! {
        <MenuItem label="Cut" disabled={selected.clone()} />
        <MenuItem label="Copy" disabled={selected.clone()} />
        <MenuItem label="Paste" disabled={paste} />
        <MenuItem label="Duplicate" disabled={selected.clone()} />
        <MenuItem label="Delete" disabled={selected.clone()} />
        <MenuItem label="Group" disabled={grouped} />
        <MenuItem label="Ungroup" disabled={ungrouped} />
        <MenuItem label="Lock" disabled={lockable} />
        <MenuItem label="Unlock" disabled={unlockable} />
        <MenuItem label="Select all" />
        <MenuItem label="Invert selection" />
        <MenuItem label="Fit selection" disabled={selected} />
    };
    view! {
        <MenuButton
            label="Actions"
            items={items}
            @test_id={"infinite-canvas.actions"}
            on_select={chosen}
        />
    }
}

#[component]
fn ZoomControls(state: Rc<CanvasState>) -> NodeId {
    let scale = state.editor().scale();
    let readout = create_memo(clone!(scale -> move || format!("{:.0}%", scale.get() * 100.0)));
    let out = clone!(state -> move || state.editor().zoom(1.0 / ZOOM_STEP));
    let reset = clone!(state scale -> move || {
        state.editor().zoom(1.0 / scale.get_untracked().max(f32::EPSILON))
    });
    let inward = clone!(state -> move || state.editor().zoom(ZOOM_STEP));
    let regioned = create_memo(clone!(state -> move || state.preview_region.get().is_none()));
    let unselected = create_memo(clone!(state -> move || state.selection.get().is_empty()));
    let chosen = clone!(state scale -> move |path: Vec<usize>| {
        let Some(index) = path.first().copied() else {
            return;
        };
        match index {
            0..=3 => {
                let wanted = ZOOM_PRESETS[index];
                state
                    .editor()
                    .zoom(wanted / scale.get_untracked().max(f32::EPSILON));
            }
            4 => state.editor().fit(),
            5 => state.request_fit_preview_region(),
            _ => state.request_fit_selection(),
        }
    });
    let items = view! {
        <MenuItem label="25%" />
        <MenuItem label="50%" />
        <MenuItem label="100%" />
        <MenuItem label="200%" />
        <MenuItem label="Fit all" />
        <MenuItem label="Fit preview region" disabled={regioned} />
        <MenuItem label="Fit selection" disabled={unselected} />
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <IconButton
                glyph={ICON_ZOOM_OUT.to_owned()}
                label="Zoom out"
                @test_id={"infinite-canvas.zoom-out"}
                on_click={out}
            />
            <Button
                label={readout}
                variant=ButtonVariant::Secondary
                @test_id={"infinite-canvas.zoom"}
                on_click={reset}
            />
            <MenuButton
                label=""
                glyph={ICON_KEYBOARD_ARROW_DOWN.to_owned()}
                arrow=false
                items={items}
                @test_id={"infinite-canvas.zoom-presets"}
                on_select={chosen}
            />
            <IconButton
                glyph={ICON_ZOOM_IN.to_owned()}
                label="Zoom in"
                @test_id={"infinite-canvas.zoom-in"}
                on_click={inward}
            />
        </List>
    }
}

#[component]
fn ImportError(state: Rc<CanvasState>) -> NodeId {
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
