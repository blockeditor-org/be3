use std::cell::Cell;

use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{
    DockBar, DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::{Editor, LoadingPane, SIDE_PANEL_WIDTH};
use beui::icons::{ICON_SCAN, ICON_WRENCH, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, NodeId, PointerPress, Rect};

use super::hotbar_ui::{HOTBAR_WIDTH, Hotbar, ToolSettings};
use super::panels::{Panels, StorageDialog};
use super::session::Session;
use super::*;
use crate::renderer::GridScene;
use beui::native::draw_gpu;

const ZOOM_STEP: f32 = 1.25;
const STAGE: &str = "stage";
const HOTBAR: &str = "hotbar";
const PANELS: &str = "panels";
const TOOLS: &str = "tools";
const ZOOM: &str = "zoom";
const PANELS_WIDTH: f32 = SIDE_PANEL_WIDTH;

#[component]
pub(super) fn LogicGridView(editor: Editor) -> NodeId {
    let session = Session::new(editor);
    let loaded = create_memo(move || session.read(LogicGridEditor::loaded));
    let waiting = create_memo(move || !loaded.get());
    let scope = NodeRef::new();
    action_scope(&scope);
    register_keys(session);
    let chrome = editor.chrome_shown();
    view! {
        <List @node_ref=&scope spacing=Space::None>
            <Show condition={waiting}>
                <ListItem grow=1.0>
                    <LoadingPane label="Loading the grid" />
                </ListItem>
            </Show>
            <Show condition={loaded}>
                <ListItem grow=1.0>
                    <List spacing=Space::None>
                        <ListItem grow=1.0>
                            <GridDock session={session} chrome={chrome} />
                        </ListItem>
                        <StorageDialog session={session} />
                    </List>
                </ListItem>
            </Show>
        </List>
    }
}

#[component]
fn GridDock(session: Session, chrome: ReadSignal<bool>) -> NodeId {
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="grid" size={SplitSize::First(HOTBAR_WIDTH)}>
                <Show condition={chrome}>
                    <DockPane id="hotbar" locked=true>
                        <DockTab id=HOTBAR title="Components">
                            <HotbarPane session={session} />
                        </DockTab>
                    </DockPane>
                </Show>
                <DockSplit id="workspace" size={SplitSize::Second(PANELS_WIDTH)}>
                    <DockPane id="stage" locked=true bare=true>
                        <DockTab id=STAGE title="Grid">
                            <GridCanvas session={session} />
                        </DockTab>
                    </DockPane>
                    <Show condition={chrome}>
                        <DockPane id="panels" locked=true>
                            <DockTab id=PANELS title="Simulation">
                                <PanelsPane session={session} />
                            </DockTab>
                        </DockPane>
                    </Show>
                </DockSplit>
            </DockSplit>
            <Show condition={chrome}>
                <DockBar
                    id=TOOLS
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <GridTools session={session} />
                </DockBar>
                <DockBar
                    id=ZOOM
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::End)}
                >
                    <GridZoom session={session} />
                </DockBar>
            </Show>
        </Docking>
    }
}

#[component]
fn HotbarPane(session: Session) -> NodeId {
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll @test_id={"logic-grid.hotbar"}>
                <Frame padding=Space::Lg>
                    <List spacing=Space::Md>
                        <Hotbar session={session} />
                        <Separator />
                        <ToolSettings session={session} />
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
fn PanelsPane(session: Session) -> NodeId {
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll @test_id={"logic-grid.panels"}>
                <Frame padding=Space::Lg>
                    <Panels session={session} />
                </Frame>
            </Scroll>
        </Frame>
    }
}

/// The grid's plain keys as actions: what a key does depends on the tool, so each runs the
/// model's `key`, and the keys that edit are off while the grid is read-only.
fn register_keys(session: Session) {
    let editable = session.editor().editable();
    let selecting = create_memo(move || session.read(|model| model.tool.kind == ToolKind::Select));
    let edits = create_memo(move || !selecting.get() || editable.get());
    let always = create_memo(|| true);
    let key = |id: &str, label: String, keys: &[Key], enabled: Memo<bool>| {
        let first = keys[0];
        let mut action = Action::new(format!("logic-grid.{id}"), label, move || {
            session.update(|model| model.key(first));
        })
        .enabled(enabled);
        for key in keys {
            action = action.shortcut(Chord::key(*key));
        }
        action.register();
    };
    Action::new("logic-grid.escape", "Cancel", move || {
        session.update(LogicGridEditor::escape);
    })
    .shortcut(Chord::key(Key::Escape))
    .register();
    let delete = [Key::Delete, Key::Backspace];
    key("delete", "Delete the selection".to_owned(), &delete, edits);
    key("rotate-left", "Rotate left".to_owned(), &[Key::Q], edits);
    key("rotate-right", "Rotate right".to_owned(), &[Key::E], edits);
    key(
        "flip-horizontal",
        "Flip horizontally".to_owned(),
        &[Key::H],
        edits,
    );
    key(
        "flip-vertical",
        "Flip vertically".to_owned(),
        &[Key::V],
        edits,
    );
    key(
        "scale-down",
        "Scale down".to_owned(),
        &[Key::BracketLeft],
        always,
    );
    key(
        "scale-up",
        "Scale up".to_owned(),
        &[Key::BracketRight],
        always,
    );
    key(
        "folder-up",
        "Up to the parent folder".to_owned(),
        &[Key::Backtick],
        always,
    );
    for (index, hotkey) in HOTBAR_KEYS.into_iter().enumerate() {
        let number = index + 1;
        key(
            &format!("hotbar.{number}"),
            format!("Choose hotbar slot {number}"),
            &[hotkey],
            always,
        );
    }
}

#[component]
fn GridTools(session: Session) -> NodeId {
    let editor = session.editor();
    let read_only = editor.read_only();
    let compile = move || {
        if let Some((id, block_type)) = session.update(|model| model.compile()) {
            editor.host().open_block(id, block_type);
        }
    };
    let challenge = create_memo(move || {
        session.read(|model| {
            model
                .challenge
                .as_ref()
                .map(|challenge| challenge.id.name().to_owned())
                .unwrap_or_default()
        })
    });
    let challenged = create_memo(move || !challenge.get().is_empty());
    let problems = create_memo(move || session.read(|model| model.grid.validate().len()));
    let misplaced = create_memo(move || problems.get() > 0);
    let problem_text = create_memo(move || format!("{} placement problems", problems.get()));
    let compile_error =
        create_memo(move || session.read(|model| model.compile_error.clone().unwrap_or_default()));
    let failed = create_memo(move || !compile_error.get().is_empty());
    view! {
        <DockBarSurface spacing=Space::Md @test_id={"logic-grid.toolbar"}>
            <Frame @test_id={"logic-grid.toolbar.grip"}>
                <DockBarGrip />
            </Frame>
            <Button
                label="Compile"
                glyph=ICON_WRENCH
                variant=ButtonVariant::Secondary
                disabled={read_only}
                @test_id={"logic-grid.compile"}
                on_click={compile}
            />
            <Show condition={challenged}>
                <Text>{challenge}</Text>
            </Show>
            <Show condition={misplaced}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{problem_text}</Text>
            </Show>
            <Show condition={failed}>
                <Text
                    style=TextStyle::CAPTION
                    color=TextColor::Danger
                    @test_id={"logic-grid.compile-error"}
                >
                    {compile_error}
                </Text>
            </Show>
        </DockBarSurface>
    }
}

#[component]
fn GridZoom(session: Session) -> NodeId {
    let editor = session.editor();
    let scale = editor.scale();
    let percent = create_memo(move || format!("{:.0}%", scale.get() * 100.0));
    let zoom_out = move || editor.zoom(1.0 / ZOOM_STEP);
    let zoom_in = move || editor.zoom(ZOOM_STEP);
    let world = editor.world();
    let recenter = move || {
        if let Some(world) = world.get_untracked() {
            let origin =
                Rect::from_center_size(Pos2::new(world.x * 0.5, world.y * 0.5), Vec2::ZERO);
            editor.fit(origin, editor.content_rect(), 1.0);
        }
    };
    view! {
        <DockBarSurface @test_id={"logic-grid.zoom"}>
            <Frame @test_id={"logic-grid.zoom.grip"}>
                <DockBarGrip />
            </Frame>
            <Button icon_only=true glyph=ICON_ZOOM_OUT label="Zoom out" on_click={zoom_out} />
            <Text style=TextStyle::CAPTION>{percent}</Text>
            <Button icon_only=true glyph=ICON_ZOOM_IN label="Zoom in" on_click={zoom_in} />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Return to the origin"
                on_click={recenter}
            />
        </DockBarSurface>
    }
}

#[component]
fn GridCanvas(session: Session) -> NodeId {
    let editor = session.editor();
    let placed_held = create_signal(Rect::ZERO);
    let placed = placed_held.read_only();
    let view = editor.canvas();
    let world = editor.world();
    let camera = create_memo(move || grid_view(view.get(), world.get(), placed.get()));
    create_effect(move || {
        let camera = camera.get();
        session.set_camera(camera);
    });
    let shown = create_memo(move || Some(camera.get()));
    let width = create_memo(move || placed.get().width());
    let height = create_memo(move || placed.get().height());

    let scene = GridScene::default();
    let pointer = session.pointer;
    let debug_hover = session.debug_hover;
    let graph_hover = session.graph_hover;
    let drawing = create_memo(move || {
        let camera = camera.get();
        let rect = placed.get();
        let pointer = pointer.get();
        let hovered = debug_hover.get();
        let graph = graph_hover.get();
        let frame =
            session.read(|model| model.render_frame(rect, camera, pointer, hovered, &graph));
        Some(scene.drawing(frame))
    });

    let labels = create_memo(move || session.read(LogicGridEditor::component_labels));
    let label_keys = create_memo(move || {
        labels.with(|labels| labels.iter().map(|label| label.key).collect::<Vec<_>>())
    });
    let zoom = create_memo(move || camera.get().scale);

    let last = store(Cell::new(None::<[f32; 2]>));
    let at = move |press: PointerPress| {
        let world = camera.get_untracked().to_canvas(press.pos);
        [world.x, world.y]
    };
    let on_press = move |press: PointerPress| {
        let world = at(press);
        last.value().set(Some(world));
        session.pointer.set(Some(world));
        let additive = press.modifiers.shift;
        session.edit(|model| model.press(world, additive));
    };
    let on_drag = move |press: PointerPress| {
        let world = at(press);
        last.value().set(Some(world));
        session.pointer.set(Some(world));
    };
    let on_active_change = move |active: bool| {
        if active {
            return;
        }
        let Some(world) = last.value().take() else {
            return;
        };
        if session.read_untracked(|model| model.gesture.is_some()) {
            session.update(|model| model.release(world));
        }
    };
    let on_secondary = move |press: PointerPress| {
        let world = at(press);
        session.edit(|model| model.delete_wire_at(world));
    };
    let on_hover_move = move |press: PointerPress| {
        session.pointer.set(Some(at(press)));
    };
    let on_hover_change = move |hovered: bool| {
        if !hovered && last.value().get().is_none() {
            session.pointer.set(None);
        }
    };

    let content = NodeRef::new();
    editor.content(&content);
    let built = view! {
        <Frame @node_ref={&content}>
            <Interactive
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
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
                    </CanvasItem>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <Canvas view={shown}>
                            <ForEach keys={label_keys}>|key: LabelKey| {
                                let label = create_memo(move || {
                                    labels.with(|labels| {
                                        labels.iter().find(|label| label.key == key).cloned()
                                    })
                                });
                                view! {
                                    <GridLabel label={label} zoom={zoom} />
                                }
                            }</ForEach>
                            <ConfiguredStorage session={session} />
                        </Canvas>
                    </CanvasItem>
                </Canvas>
            </Interactive>
        </Frame>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed_held.set(measured.rect);
        }
    });
    built
}

#[component]
fn ConfiguredStorage(session: Session) -> CanvasItem {
    let bounds = create_memo(move || {
        session.read(|model| {
            let component = model.grid.component(model.configured_storage?)?;
            let size = component.size()?;
            let left = component.position.x as f32;
            let top = component.position.y as f32;
            Some([left, top, size.width as f32, size.height as f32])
        })
    });
    let x = create_memo(move || bounds.get().map_or(0.0, |bounds| bounds[0]));
    let y = create_memo(move || bounds.get().map_or(0.0, |bounds| bounds[1]));
    let width = create_memo(move || bounds.get().map_or(0.0, |bounds| bounds[2]));
    let height = create_memo(move || bounds.get().map_or(0.0, |bounds| bounds[3]));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame @node_ref={&session.configured_at} />
        </CanvasItem>
    }
}

#[component]
fn GridLabel(label: Memo<Option<ComponentLabel>>, zoom: Memo<f32>) -> CanvasItem {
    let rect = create_memo(move || label.get().map_or([0.0; 4], |label| label.rect));
    let x = create_memo(move || rect.get()[0]);
    let y = create_memo(move || rect.get()[1]);
    let width = create_memo(move || rect.get()[2] - rect.get()[0]);
    let height = create_memo(move || rect.get()[3] - rect.get()[1]);
    let text = create_memo(move || label.get().map(|label| label.text).unwrap_or_default());
    let size = create_memo(move || {
        TextSize::Custom(
            label
                .get()
                .map_or(LabelKind::Port.font_size(zoom.get()), |label| {
                    label.kind.font_size(zoom.get())
                }),
        )
    });
    let color = create_memo(move || {
        TextColor::Custom((label.get().map_or(LABEL_COLOR, |label| label.kind.color())).into())
    });
    let align = create_memo(move || label.get().map_or(TextAlign::Center, |label| label.align));
    let vertical_align = create_memo(move || {
        label
            .get()
            .map_or(TextAlign::Center, |label| label.vertical_align)
    });
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Text size color={color} align={align} vertical_align={vertical_align}>{text}</Text>
        </CanvasItem>
    }
}
