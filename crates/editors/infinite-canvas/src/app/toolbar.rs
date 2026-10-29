use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::dock::styled::{DockBarGrip, DockingLayout, use_dock_bar};
use block_editor_beui::beui::dock::unstyled::{DockBarPlacement, DockEdge};
use block_editor_beui::sheet_open;
use infinite_canvas_types::CanvasProfile;

use super::actions::CanvasActions;
use super::selection_bar::SelectionTools;
use super::state::{CanvasState, Tool};
use beui::{Align, Align2, Direction, Vec2};

const TOOLS_SHOWN: [(Tool, &str); 10] = [
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
const PILL_MARGIN: Space = Space::Lg;
const BAR_MARGIN: Space = Space::Md;

pub(crate) const STAGE: &str = "stage";
pub(crate) const TOOLS: &str = "tools";

pub(crate) fn tools_open(state: CanvasState, shown: ReadSignal<bool>) -> Memo<bool> {
    let previewing = state.previewing();
    let sheet = sheet_open();
    create_memo(move || shown.get() && !previewing && !sheet.get())
}

#[component]
pub(crate) fn ToolBar(state: CanvasState, open: Memo<bool>, size: Signal<Vec2>) -> NodeId {
    let bar = use_dock_bar();
    let direction = create_memo(move || bar.map_or(Direction::Horizontal, |bar| bar.direction()));
    let across = create_memo(move || match direction.get() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    });
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let tool_keys = create_memo(move || {
        let profile = state.profile.get();
        (0..TOOLS_SHOWN.len())
            .filter(|index| TOOLS_SHOWN[*index].0.belongs_to(profile))
            .collect::<Vec<usize>>()
    });
    let block_button = NodeRef::new();
    let add_block = move || state.open_block_picker_at(block_button);
    create_effect(move || {
        if !open.get() && size.get_untracked() != Vec2::ZERO {
            size.set(Vec2::ZERO);
        }
    });
    let content = Render::new(move |()| {
        let tools = actions.clone();
        let pill = view! {
            <ToolPill direction={direction} @test_id={"infinite-canvas.dock"}>
                <Frame @test_id={"infinite-canvas.dock.grip"}>
                    <DockBarGrip />
                </Frame>
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
                <Button
                    icon_only=true
                    @node_ref={&block_button}
                    label="Block"
                    glyph={actions.add_block.glyph().to_owned()}
                    disabled={actions.add_block.disabled()}
                    @test_id={"infinite-canvas.dock.block"}
                    on_click={add_block}
                />
                <Separator direction={across} length=Size::Md />
                <ToggleButton
                    icon_only=true
                    action={actions.ignore_touch.clone()}
                    @test_id={"infinite-canvas.dock.ignore-touch"}
                />
            </ToolPill>
        };
        let measured = measure(pill);
        create_effect(move || {
            let shown = match open.get() {
                true => measured.get().map_or(Vec2::ZERO, |measured| measured.size),
                false => Vec2::ZERO,
            };
            if size.get_untracked() != shown {
                size.set(shown);
            }
        });
        pill
    })
    .once();
    let beside = create_memo(move || {
        bar.is_some_and(|bar| matches!(bar.placement(), DockBarPlacement::Beside(_)))
    });
    let padding = create_memo(move || match beside.get() {
        true => Padding::all(Space::Sm),
        false => Padding::ZERO,
    });
    view! {
        <Frame padding={padding}>
            <List spacing=Space::None>
                <ShowKeepAlive condition={open}>
                    {content.call(())}
                </ShowKeepAlive>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn SelectionDock(
    state: CanvasState,
    layout: DockingLayout<&'static str>,
    open: Memo<bool>,
    bar: ReadSignal<Vec2>,
) -> NodeId {
    let placement = create_memo(move || {
        layout
            .bar_placement(&TOOLS)
            .unwrap_or(DockBarPlacement::Over(DockEdge::Bottom))
    });
    let edge = create_memo(move || placement.get().edge());
    let direction = create_memo(move || edge.get().direction());
    let chosen = create_memo(move || {
        let tool = state.tool.get();
        open.get()
            && (!state.selection.get().is_empty()
                || tool.draws_a_line()
                || (state.profile.get() == CanvasProfile::Prototyping && tool.styles()))
    });
    let align = create_memo(move || match edge.get() {
        DockEdge::Top => Align2::new(Align::Center, Align::Start),
        DockEdge::Bottom => Align2::new(Align::Center, Align::End),
        DockEdge::Left => Align2::new(Align::Start, Align::Center),
        DockEdge::Right => Align2::new(Align::End, Align::Center),
    });
    let padding = create_memo(move || {
        let away = match placement.get() {
            DockBarPlacement::Over(edge) => {
                let bar = bar.get();
                let thickness = match edge.direction() {
                    Direction::Horizontal => bar.y,
                    Direction::Vertical => bar.x,
                };
                Space::Custom(BAR_MARGIN.px() + thickness + PILL_MARGIN.px())
            }
            _ => BAR_MARGIN,
        };
        let sides = Padding::all(BAR_MARGIN);
        match edge.get() {
            DockEdge::Top => Padding { top: away, ..sides },
            DockEdge::Bottom => Padding {
                bottom: away,
                ..sides
            },
            DockEdge::Left => Padding { left: away, ..sides },
            DockEdge::Right => Padding {
                right: away,
                ..sides
            },
        }
    });
    view! {
        <Frame align={align} padding={padding}>
            <List spacing=Space::None>
                <Show condition={chosen}>
                    <SelectionTools state direction />
                </Show>
            </List>
        </Frame>
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
    let content = Render::new(move |()| {
        view! {
            <Frame padding=PILL_MARGIN>
                <ToolPill @test_id={"infinite-canvas.zoom-pill"}>
                    <Button
                        icon_only=true
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
                    <Button
                        icon_only=true
                        @test_id={"infinite-canvas.pill.zoom-in"}
                        action={actions.zoom_in}
                    />
                    <Button
                        icon_only=true
                        @test_id={"infinite-canvas.pill.fit"}
                        action={actions.fit_all}
                    />
                </ToolPill>
            </Frame>
        }
    })
    .once();
    view! {
        <Floating anchor={anchor} edge=Edge::TopEnd open={open} keep_alive=true>
            {content.call(())}
        </Floating>
    }
}

#[component]
fn ToolChoice(state: CanvasState, actions: CanvasActions, index: usize) -> NodeId {
    let (tool, name) = TOOLS_SHOWN[index];
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
    view! {
        <Banner
            message={state.import_error}
            id="infinite-canvas.import-error"
            on_dismiss={move || state.dismiss_import_error()}
        />
    }
}
