use std::cell::Cell;

use beui::icons::{ICON_SCAN, ICON_WRENCH, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, KeyPress, NodeId, PointerPress, Rect};
use block_editor_beui::{Editor, LoadingPane, Side, Sidebar, Toolbar};

use super::hotbar_ui::{HOTBAR_WIDTH, Hotbar, ToolSettings};
use super::panels::Panels;
use super::session::Session;
use super::*;
use crate::renderer::GridScene;
use beui::{Direction, ItemSize};

const ZOOM_STEP: f32 = 1.25;

#[component]
pub(super) fn LogicGridView(editor: Editor) -> NodeId {
    let session = Session::new(editor);
    let loaded = create_memo(move || session.read(LogicGridEditor::loaded));
    let waiting = create_memo(move || !loaded.get());
    on_shortcut(move |press: KeyPress| shortcut(session, press));
    let chrome = editor.chrome_shown();
    view! {
        <List spacing=Space::None>
            <Show condition={waiting}>
                <LoadingPane @sizing=ItemSize::Percent(100.0) label="Loading the grid" />
            </Show>
            <Show condition={loaded}>
                <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                    <GridToolbar session={session} shown={chrome} />
                    <List
                        @sizing=ItemSize::Percent(100.0)
                        direction=Direction::Horizontal
                        spacing=Space::None
                    >
                        <Sidebar side=Side::Left shown={chrome} width=HOTBAR_WIDTH>
                            <Hotbar session={session} />
                            <Separator />
                            <ToolSettings session={session} />
                        </Sidebar>
                        <GridCanvas @sizing=ItemSize::Percent(100.0) session={session} />
                        <Sidebar shown={chrome}>
                            <Panels session={session} />
                        </Sidebar>
                    </List>
                </List>
            </Show>
        </List>
    }
}

fn shortcut(session: Session, press: KeyPress) -> bool {
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
    ) && session.read_untracked(|model| model.tool.kind == ToolKind::Select);
    if edits && !editable {
        return false;
    }
    session.update(|model| model.key(press.key))
}

#[component]
fn GridToolbar(session: Session, shown: ReadSignal<bool>) -> DynamicSegment<ListChild> {
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
    let problem_color = TextColor::Danger;
    let error_color = TextColor::Danger;
    view! {
        <Toolbar shown={shown}>
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
                <Text style=TextStyle::CAPTION color={problem_color}>{problem_text}</Text>
            </Show>
            <Show condition={failed}>
                <Text
                    style=TextStyle::CAPTION
                    color={error_color}
                    @test_id={"logic-grid.compile-error"}
                >
                    {compile_error}
                </Text>
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Button icon_only=true glyph=ICON_ZOOM_OUT label="Zoom out" on_click={zoom_out} />
            <Text style=TextStyle::CAPTION>{percent}</Text>
            <Button icon_only=true glyph=ICON_ZOOM_IN label="Zoom in" on_click={zoom_in} />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Return to the origin"
                on_click={recenter}
            />
        </Toolbar>
    }
}

#[component]
fn GridCanvas(session: Session) -> NodeId {
    let editor = session.editor();
    view! {
        <PlacementBuilder>|placed: ReadSignal<Rect>| {
            let view = editor.canvas();
            let world = editor.world();
            let camera = create_memo(move || {
                Camera::from_view(view.get(), world.get(), placed.get())
            });
            create_effect(move || {
                let camera = camera.get();
                session.set_camera(camera);
            });
            let grid_view = create_memo(move || {
                Some(camera.get().view(placed.get()))
            });
            let width = create_memo(move || placed.get().width());
            let height = create_memo(move || placed.get().height());

            let scene = GridScene::default();
            let pointer = session.pointer;
            let debug_hover = session.debug_hover;
            let graph_hover = session.graph_hover;
            let drawing = create_memo(move || {
                let camera = camera.get();
                let size = placed.get().size();
                let pointer = pointer.get();
                let hovered = debug_hover.get();
                let graph = graph_hover.get();
                let frame = session.read(|model| model.render_frame(size, camera, pointer, hovered, &graph));
                Some(scene.drawing(frame))
            });

            let labels =
                create_memo(move || session.read(LogicGridEditor::component_labels));
            let label_keys = create_memo(move || {
                labels.with(|labels| labels.iter().map(|label| label.key).collect::<Vec<_>>())
            });
            let zoom = create_memo(move || camera.get().zoom);

            let last = store(Cell::new(None::<[f32; 2]>));
            let at = move |press: PointerPress| {
                camera.get_untracked().screen_to_world(press.pos, placed.get_untracked())
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
            view! {
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
                                <Canvas view={grid_view}>
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
                                </Canvas>
                            </CanvasItem>
                        </Canvas>
                    </Interactive>
                </Frame>
            }
        }</PlacementBuilder>
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
        TextColor::Custom(label.get().map_or(LABEL_COLOR, |label| label.kind.color()))
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
