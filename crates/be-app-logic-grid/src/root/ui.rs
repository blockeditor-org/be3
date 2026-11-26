use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{DockBar, DockPane, DockSplit, DockTab, Docking, SplitSize};
use be_sdk_beui::{Editor, LoadingPane, ZoomBar};
use beui::icons::ICON_WRENCH;
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, NodeId, PointerPress, Rect};

use super::hotbar_ui::{HOTBAR_WIDTH, Hotbar, ToolSettings};
use super::panels::{Panels, StorageDialog};
use super::session::Session;
use super::*;
use crate::renderer::GridScene;
use beui::native::draw_gpu;

const STAGE: &str = "stage";
const TOOLS: &str = "tools";
const ZOOM: &str = "zoom";

#[component]
pub(super) fn LogicGridView(editor: Editor) -> NodeId {
    let session = Session::new(editor);
    let loaded = create_memo(move || session.read(LogicGridEditor::loaded));
    let scope = NodeRef::new();
    action_scope(&scope);
    register_keys(session);
    let chrome = editor.chrome_shown();
    view! {
        <List @node_ref=&scope spacing=Space::None>
            <Show
                value={loaded}
                fallback={|| view! {
                    <ListItem grow=1.0>
                        <LoadingPane label="Loading the grid" />
                    </ListItem>
                }}
            >
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
    // The grid's fit returns the view to its origin, at 100%.
    let editor = session.editor();
    let world = editor.world();
    editor.fit_with(move || {
        if let Some(world) = world.get_untracked() {
            let origin =
                Rect::from_center_size(Pos2::new(world.x * 0.5, world.y * 0.5), Vec2::ZERO);
            editor.fit(origin, editor.content_rect(), 1.0);
        }
    });
    view! {
        <Docking>
            <DockSplit size={SplitSize::First(HOTBAR_WIDTH)}>
                <DockPane chrome=true>
                    <DockTab title="Components">
                        <List spacing=Space::Md @test_id={"logic-grid.hotbar"}>
                            <Hotbar session={session} />
                            <Separator />
                            <ToolSettings session={session} />
                        </List>
                    </DockTab>
                </DockPane>
                <DockSplit>
                    <DockPane stage=true>
                        <DockTab id=STAGE>
                            <GridCanvas session={session} />
                        </DockTab>
                    </DockPane>
                    <DockPane chrome=true>
                        <DockTab title="Simulation">
                            <Panels session={session} />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </DockSplit>
            <Show value={chrome}>
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
                    <ZoomBar editor={session.editor()} />
                </DockBar>
            </Show>
        </Docking>
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
    let problems = create_memo(move || session.read(|model| model.grid.validate().len()));
    let misplaced = create_memo(move || problems.get() > 0);
    let problem_text = create_memo(move || format!("{} placement problems", problems.get()));
    let compile_error =
        create_memo(move || session.read(|model| model.compile_error.clone().unwrap_or_default()));
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
            <Show value={move || challenge.with(|challenge| !challenge.is_empty())}>
                <Text>{challenge}</Text>
            </Show>
            <Show value={misplaced}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{problem_text}</Text>
            </Show>
            <Show value={move || compile_error.with(|error| !error.is_empty())}>
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
fn GridCanvas(session: Session) -> NodeId {
    let editor = session.editor();
    let placed = create_signal(Rect::ZERO);
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
    let zoom = create_memo(move || camera.get().scale);

    let last = StoredCell::new(None::<[f32; 2]>);
    let at = move |press: PointerPress| {
        let world = camera.get_untracked().to_canvas(press.pos);
        [world.x, world.y]
    };
    let on_press = move |press: PointerPress| {
        let world = at(press);
        last.set(Some(world));
        session.pointer.set(Some(world));
        let additive = press.modifiers.shift;
        session.edit(|model| model.press(world, additive));
    };
    let on_drag = move |press: PointerPress| {
        let world = at(press);
        last.set(Some(world));
        session.pointer.set(Some(world));
    };
    let on_active_change = move |active: bool| {
        if active {
            return;
        }
        let Some(world) = last.take() else {
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
        if !hovered && last.get().is_none() {
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
                        <Drawing draw={move || draw_gpu(drawing.get())} />
                    </CanvasItem>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <Canvas view={shown}>
                            <ForEach
                                value={labels}
                                key={|label: &ComponentLabel| label.key}
                            >|label: ReadSignal<ComponentLabel>|
                                <GridLabel label={label} zoom={zoom} />
                            </ForEach>
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
            placed.set(measured.rect);
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
fn GridLabel(label: ReadSignal<ComponentLabel>, zoom: Memo<f32>) -> CanvasItem {
    let rect = create_memo(move || label.with(|label| label.rect));
    let x = create_memo(move || rect.get()[0]);
    let y = create_memo(move || rect.get()[1]);
    let width = create_memo(move || rect.get()[2] - rect.get()[0]);
    let height = create_memo(move || rect.get()[3] - rect.get()[1]);
    let text = create_memo(move || label.with(|label| label.text.clone()));
    let size =
        create_memo(move || TextSize::Custom(label.with(|label| label.kind).font_size(zoom.get())));
    let color =
        create_memo(move || TextColor::Custom(label.with(|label| label.kind.color()).into()));
    let align = create_memo(move || label.with(|label| label.align));
    let vertical_align = create_memo(move || label.with(|label| label.vertical_align));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Text size color={color} align={align} vertical_align={vertical_align}>{text}</Text>
        </CanvasItem>
    }
}
