use std::cell::Cell;

use beui::icons::{ICON_BUILD, ICON_FIT_SCREEN, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use beui::reactive::{
    Canvas, CanvasItem, ClickCatcher, Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef,
    ReadSignal, Show, Spacer, Text, Viewport, clone, component, component_rect, create_effect,
    create_memo, focus_takes_text, on_shortcut, view,
};
use beui::styled::{Body, Button, ButtonVariant, Caption, IconButton, Separator, use_theme};
use beui::{CursorIcon, KeyPress, NodeId, PointerPress};
use block_editor_beui::{Editor, Side, Sidebar, Toolbar};

use super::hotbar_ui::{HOTBAR_WIDTH, Hotbar, ToolSettings};
use super::panels::Panels;
use super::session::Session;
use super::*;
use crate::renderer::GridScene;

const ZOOM_STEP: f32 = 1.25;

#[component]
pub(super) fn LogicGridView(editor: Editor) -> NodeId {
    let session = Session::new(&editor);
    let loaded = create_memo(clone!(session -> move || session.read(LogicGridEditor::loaded)));
    let waiting = create_memo(clone!(loaded -> move || !loaded.get()));
    let shortcuts = Rc::clone(&session);
    on_shortcut(move |press: KeyPress| shortcut(&shortcuts, press));
    let chrome = editor.chrome_shown();
    let left_chrome = chrome.clone();
    let right_chrome = chrome.clone();
    let hotbar_session = Rc::clone(&session);
    let settings_session = Rc::clone(&session);
    let canvas_session = Rc::clone(&session);
    let panels_session = Rc::clone(&session);
    view! {
        <List spacing=0.0>
            <Show condition={waiting}>
                <Frame padding_horizontal=16.0 padding_vertical=16.0>
                    <Caption content="Loading the grid" />
                </Frame>
            </Show>
            <Show condition={loaded}>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <GridToolbar session={Rc::clone(&session)} shown={chrome} />
                    <List
                        @sizing=ItemSize::Percent(100.0)
                        direction=Direction::Horizontal
                        spacing=0.0
                    >
                        <Sidebar side=Side::Left shown={left_chrome} width=HOTBAR_WIDTH>
                            <Hotbar session={hotbar_session} />
                            <Separator />
                            <ToolSettings session={settings_session} />
                        </Sidebar>
                        <GridCanvas @sizing=ItemSize::Percent(100.0) session={canvas_session} />
                        <Sidebar shown={right_chrome}>
                            <Panels session={panels_session} />
                        </Sidebar>
                    </List>
                </List>
            </Show>
        </List>
    }
}

fn shortcut(session: &Rc<Session>, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt || focus_takes_text() {
        return false;
    }
    if press.key == Key::Escape {
        session.update(LogicGridEditor::escape);
        return true;
    }
    let editable = session.editable();
    let edits = matches!(
        press.key,
        Key::Delete | Key::Backspace | Key::Q | Key::E | Key::H | Key::V
    ) && session.peek(|model| model.tool.kind == ToolKind::Select);
    if edits && !editable {
        return false;
    }
    session.update(|model| model.key(press.key))
}

#[component]
fn GridToolbar(session: Rc<Session>, shown: ReadSignal<bool>) -> NodeId {
    let editor = session.editor().clone();
    let read_only = editor.read_only();
    let compile = clone!(session editor -> move || {
        if let Some((id, block_type)) = session.update(|model| model.compile()) {
            editor.host().open_block(id, block_type);
        }
    });
    let challenge = create_memo(clone!(session -> move || {
        session.read(|model| {
            model
                .challenge
                .as_ref()
                .map(|challenge| challenge.id.name().to_owned())
                .unwrap_or_default()
        })
    }));
    let challenged = create_memo(clone!(challenge -> move || !challenge.get().is_empty()));
    let problems = create_memo(clone!(session -> move || {
        session.read(|model| model.grid.validate().len())
    }));
    let misplaced = create_memo(clone!(problems -> move || problems.get() > 0));
    let problem_text = create_memo(clone!(problems -> move || {
        format!("{} placement problems", problems.get())
    }));
    let compile_error = create_memo(clone!(session -> move || {
        session.read(|model| model.compile_error.clone().unwrap_or_default())
    }));
    let failed = create_memo(clone!(compile_error -> move || !compile_error.get().is_empty()));
    let scale = editor.scale();
    let percent = create_memo(clone!(scale -> move || format!("{:.0}%", scale.get() * 100.0)));
    let zoom_out = clone!(editor -> move || editor.zoom(1.0 / ZOOM_STEP));
    let zoom_in = clone!(editor -> move || editor.zoom(ZOOM_STEP));
    let recenter = clone!(editor -> move || editor.fit());
    let theme = use_theme();
    let problem_color = theme.danger.clone();
    let error_color = theme.danger.clone();
    view! {
        <Toolbar shown={shown}>
            <Button
                label="Compile"
                glyph={ICON_BUILD.to_owned()}
                variant=ButtonVariant::Secondary
                disabled={read_only}
                @test_id={"logic-grid.compile"}
                on_click={compile}
            />
            <Show condition={challenged}>
                <Body content={challenge} />
            </Show>
            <Show condition={misplaced}>
                <Caption content={problem_text} color={problem_color} />
            </Show>
            <Show condition={failed}>
                <Caption
                    content={compile_error}
                    color={error_color}
                    @test_id={"logic-grid.compile-error"}
                />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <IconButton glyph={ICON_ZOOM_OUT.to_owned()} label="Zoom out" on_click={zoom_out} />
            <Caption content={percent} />
            <IconButton glyph={ICON_ZOOM_IN.to_owned()} label="Zoom in" on_click={zoom_in} />
            <IconButton
                glyph={ICON_FIT_SCREEN.to_owned()}
                label="Return to the origin"
                on_click={recenter}
            />
        </Toolbar>
    }
}

#[component]
fn GridCanvas(session: Rc<Session>) -> NodeId {
    let editor = session.editor().clone();
    let placed = component_rect();
    let view = editor.canvas();
    let world = editor.world();
    let camera = create_memo(clone!(placed -> move || {
        Camera::from_view(view.get(), world.get(), placed.get())
    }));
    create_effect(clone!(session camera -> move || {
        let camera = camera.get();
        session.set_camera(camera);
    }));
    let grid_view = create_memo(clone!(camera placed -> move || {
        Some(camera.get().view(placed.get()))
    }));
    let width = create_memo(clone!(placed -> move || placed.get().width()));
    let height = create_memo(clone!(placed -> move || placed.get().height()));

    let scene = GridScene::default();
    let pointer = session.pointer.clone();
    let debug_hover = session.debug_hover.clone();
    let graph_hover = session.graph_hover.clone();
    let drawing = create_memo(clone!(session camera placed -> move || {
        let camera = camera.get();
        let size = placed.get().size();
        let pointer = pointer.get();
        let hovered = debug_hover.get();
        let graph = graph_hover.get();
        let frame = session.read(|model| model.render_frame(size, camera, pointer, hovered, &graph));
        Some(scene.drawing(frame))
    }));

    let labels =
        create_memo(clone!(session -> move || session.read(LogicGridEditor::component_labels)));
    let label_keys = create_memo(clone!(labels -> move || {
        labels.with(|labels| labels.iter().map(|label| label.key).collect::<Vec<_>>())
    }));
    let zoom = create_memo(clone!(camera -> move || camera.get().zoom));

    let last = Rc::new(Cell::new(None::<[f32; 2]>));
    let at = clone!(camera placed -> move |press: PointerPress| {
        camera.get_untracked().screen_to_world(press.pos, placed.get_untracked())
    });
    let on_press = clone!(session at last -> move |press: PointerPress| {
        let world = at(press);
        last.set(Some(world));
        session.set_pointer.set(Some(world));
        let additive = press.modifiers.shift;
        session.edit(|model| model.press(world, additive));
    });
    let on_drag = clone!(session at last -> move |press: PointerPress| {
        let world = at(press);
        last.set(Some(world));
        session.set_pointer.set(Some(world));
    });
    let on_active_change = clone!(session last -> move |active: bool| {
        if active {
            return;
        }
        let Some(world) = last.take() else {
            return;
        };
        if session.peek(|model| model.gesture.is_some()) {
            session.update(|model| model.release(world));
        }
    });
    let on_secondary = clone!(session at -> move |press: PointerPress| {
        let world = at(press);
        session.edit(|model| model.delete_wire_at(world));
    });
    let on_hover_move = clone!(session at -> move |press: PointerPress| {
        session.set_pointer.set(Some(at(press)));
    });
    let on_hover_change = clone!(session last -> move |hovered: bool| {
        if !hovered && last.get().is_none() {
            session.set_pointer.set(None);
        }
    });

    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Frame @node_ref={&content}>
            <ClickCatcher
                cursor=CursorIcon::Crosshair
                @test_id={"logic-grid.canvas"}
                on_press={on_press}
                on_drag={on_drag}
                on_active_change={on_active_change}
                on_secondary_press={on_secondary}
                on_hover_move={on_hover_move}
                on_hover_change={on_hover_change}
            >
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width={width.clone()} height={height.clone()}>
                        <Viewport drawing={drawing} />
                    </CanvasItem>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <Canvas view={grid_view}>
                            <ForEach keys={label_keys}>
                                {move |key: LabelKey| {
                                    let label = create_memo(clone!(labels -> move || {
                                        labels.with(|labels| {
                                            labels.iter().find(|label| label.key == key).cloned()
                                        })
                                    }));
                                    view! {
                                        <GridLabel label={label} zoom={zoom.clone()} />
                                    }
                                }}
                            </ForEach>
                        </Canvas>
                    </CanvasItem>
                </Canvas>
            </ClickCatcher>
        </Frame>
    }
}

#[component]
fn GridLabel(label: Memo<Option<ComponentLabel>>, zoom: Memo<f32>) -> CanvasItem {
    let rect = create_memo(clone!(label -> move || {
        label.get().map_or([0.0; 4], |label| label.rect)
    }));
    let x = create_memo(clone!(rect -> move || rect.get()[0]));
    let y = create_memo(clone!(rect -> move || rect.get()[1]));
    let width = create_memo(clone!(rect -> move || rect.get()[2] - rect.get()[0]));
    let height = create_memo(clone!(rect -> move || rect.get()[3] - rect.get()[1]));
    let text = create_memo(
        clone!(label -> move || label.get().map(|label| label.text).unwrap_or_default()),
    );
    let size = create_memo(clone!(label zoom -> move || {
        label
            .get()
            .map_or(LabelKind::Port.font_size(zoom.get()), |label| label.kind.font_size(zoom.get()))
    }));
    let color = create_memo(clone!(label -> move || {
        label.get().map_or(LABEL_COLOR, |label| label.kind.color())
    }));
    let align = create_memo(clone!(label -> move || {
        label.get().map_or(TextAlign::Center, |label| label.align)
    }));
    let vertical_align = create_memo(clone!(label -> move || {
        label.get().map_or(TextAlign::Center, |label| label.vertical_align)
    }));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Text
                string={text}
                font_size={size}
                color={color}
                align={align}
                vertical_align={vertical_align}
            />
        </CanvasItem>
    }
}
