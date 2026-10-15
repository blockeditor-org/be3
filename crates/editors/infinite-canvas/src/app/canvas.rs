use std::rc::Rc;

use block_editor_beui::be_block::canvas::{CanvasEntity, CanvasEntityKind};
use std::cell::RefCell;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{KeyPress, NodeId, Painter, PointerPress, Rect, Vec2, pos2};
use block_editor_beui::{ChildBlock, ChildMode, ChildState, ChildTarget, ViewChange};
use uuid::Uuid;

use crate::geometry::*;

use super::menu::CanvasMenu;
use super::overlay::{Artboard, Backdrop, Overlay};
use super::paint::{Camera, EntityPaint, Palette, resolve_color};
use super::state::CanvasState;

const ITEM_MARGIN: f32 = 2.0;

#[component]
pub(crate) fn CanvasStage(state: CanvasState) -> NodeId {
    view! {
        <PlacementBuilder>
            {move |placed: ReadSignal<Rect>| {
                let parent = state.editor().placed_content();
                let shown = create_memo(move || match parent.get() {
                    Some(parent) => parent.intersect(placed.get()),
                    None => placed.get(),
                });
                create_effect(move || state.set_stage(shown.get()));

                let view = state.editor().canvas();
                let world = state.editor().world();
                let camera = create_memo(move || {
                    super::state::camera_view(view.get(), world.get(), placed.get(), state.framed())
                });
                let ids = create_memo(move || {
                    state
                        .displayed()
                        .iter()
                        .map(|entity| entity.id)
                        .collect::<Vec<Uuid>>()
                });
                let embeds = ids;
                let shape_camera = camera;
                let overlay_camera = camera;
                let overlay_stage = shown;
                let backdrop_camera = camera;
                let backdrop_stage = shown;
                let inset = create_memo(move || {
                    let (placed, shown) = (placed.get(), shown.get());
                    Sides::new(
                        (shown.left() - placed.left()).max(0.0),
                        (shown.top() - placed.top()).max(0.0),
                        (placed.right() - shown.right()).max(0.0),
                        (placed.bottom() - shown.bottom()).max(0.0),
                    )
                });
                let placement = camera;
                let previewing = state.previewing();
                view! {
                    <CanvasMenu disabled={previewing}>
                        <Frame padding={inset}>
                            <CanvasSurface state={state}>
                                <Canvas
                                    view={create_memo(move || Some(placement.get()))}
                                    @test_id={"infinite-canvas.canvas"}
                                >
                                    <CanvasBackdrop
                                        state={state}
                                        camera={backdrop_camera}
                                        stage={backdrop_stage}
                                    />
                                    <ForEach keys={ids}>
                                        {move |id: Uuid| {
                                            let camera = shape_camera;
                                            view! {
                                                <EntityShape state id camera />
                                            }
                                        }}
                                    </ForEach>
                                    <ForEach keys={embeds}>
                                        {move |id: Uuid| {
                                            view! {
                                                <EntityEmbed state id />
                                            }
                                        }}
                                    </ForEach>
                                    <Show condition={!previewing}>
                                        <CanvasOverlay
                                            state={state}
                                            camera={overlay_camera}
                                            stage={overlay_stage}
                                        />
                                    </Show>
                                </Canvas>
                            </CanvasSurface>
                        </Frame>
                    </CanvasMenu>
                }
            }}
        </PlacementBuilder>
    }
}

#[component]
fn CanvasSurface(state: CanvasState, children: Option<Child>) -> NodeId {
    let cursor = create_memo(move || state.cursor());
    let interactive = !state.previewing();
    view! {
        <Interactive
            focusable=true
            tab_stop={interactive}
            on_key={move |press: KeyPress| state.key(press)}
            pastes={vec![PasteKind::Image, PasteKind::Text]}
            on_paste={move |pasted: Pasted| state.take_paste(pasted)}
            cursor={cursor}
            touch_drags={interactive}
            on_cancel={move || state.cancel()}
            on_press={move |press: PointerPress| state.press(press)}
            on_secondary_press={move |press: PointerPress| state.secondary_press(press)}
            on_drag={move |press: PointerPress| state.drag(press)}
            on_hover_move={move |press: PointerPress| {
                let at = state.world_at(press.pos);
                state.hover(Some(at));
            }}
            on_hover_change={move |over: bool| {
                if !over {
                    state.hover(None);
                }
            }}
            on_active_change={move |active: bool| {
                if !active {
                    state.release();
                }
            }}
            children={children}
        />
    }
}

#[component]
fn CanvasOverlay(state: CanvasState, camera: Memo<CanvasView>, stage: Memo<Rect>) -> CanvasItem {
    let visible = create_memo(move || camera.get().rect_to_canvas(stage.get()));
    let theme = use_theme();
    let (draw, set_draw) = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<Overlay>);
    create_effect(move || {
        let overlay = Overlay {
            camera: Camera::of(Some(camera.get()), stage.get().min),
            palette: palette(&theme),
            stage: stage.get(),
            empty: state.entities.get().is_empty(),
            gesture: state.gesture.get(),
            frame: super::state::selection_frame(&state.displayed(), &state.selection.get()),
            line: state.selected_line().and_then(|line| {
                let shown = state
                    .displayed()
                    .into_iter()
                    .find(|entity| entity.id == line.id)?;
                Some(line_ends(&shown))
            }),
            resize: state.selection_handles().0,
            rotatable: state.selection_has_unlocked() && state.selection_allows_rotation(),
            presence: state.presence.get(),
            tool: state.tool.get(),
            pointer: state.pointer.get(),
        };
        if shown.borrow().as_ref() == Some(&overlay) {
            return;
        }
        *shown.borrow_mut() = Some(overlay.clone());
        set_draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| {
            overlay.draw(&painter.in_document())
        }));
    });
    let x = create_memo(move || visible.get().left());
    let y = create_memo(move || visible.get().top());
    let width = create_memo(move || visible.get().width().max(1.0));
    let height = create_memo(move || visible.get().height().max(1.0));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn CanvasBackdrop(state: CanvasState, camera: Memo<CanvasView>, stage: Memo<Rect>) -> CanvasItem {
    let visible = create_memo(move || camera.get().rect_to_canvas(stage.get()));
    let theme = use_theme();
    let (draw, set_draw) = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<Backdrop>);
    create_effect(move || {
        let artboards = state
            .displayed()
            .into_iter()
            .filter(|entity| !entity.style.hidden)
            .filter_map(|entity| match &entity.kind {
                CanvasEntityKind::Artboard { name } => Some(Artboard {
                    bounds: entity_bounds(&entity),
                    name: name.clone(),
                    fill: resolve_color(
                        entity.style.fill.unwrap_or_default(),
                        theme.background.get(),
                    ),
                }),
                _ => None,
            })
            .collect();
        let backdrop = Backdrop {
            camera: Camera::of(Some(camera.get()), stage.get().min),
            stage: stage.get(),
            label: theme.text_muted.get(),
            chrome: !state.previewing(),
            artboards,
        };
        if shown.borrow().as_ref() == Some(&backdrop) {
            return;
        }
        *shown.borrow_mut() = Some(backdrop.clone());
        set_draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| {
            backdrop.draw(&painter.in_document())
        }));
    });
    let x = create_memo(move || visible.get().left());
    let y = create_memo(move || visible.get().top());
    let width = create_memo(move || visible.get().width().max(1.0));
    let height = create_memo(move || visible.get().height().max(1.0));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height} @test_id={"infinite-canvas.backdrop"}>
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn EntityShape(state: CanvasState, id: Uuid, camera: Memo<CanvasView>) -> CanvasItem {
    let entity = entity_of(state, id);
    let bounds = create_memo(move || {
        entity
            .get()
            .map(|entity| item_bounds(&entity))
            .unwrap_or(Rect::ZERO)
    });
    let thumbhash = create_memo(move || state.thumbhash_of(reference_of(&entity.get()?)?));
    let placeholder = create_memo(move || thumbhash.get().and_then(|thumbhash| thumbhash.decode()));
    let theme = use_theme();
    let (draw, set_draw) = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<EntityPaint>);
    create_effect(move || {
        let Some(entity) = entity.get() else {
            return;
        };
        let reference = reference_of(&entity);
        let label = reference.and_then(|reference| state.label_of(reference));
        let loading = reference.is_some();
        let paint = EntityPaint {
            covered: state.child_state(entity.id).available,
            camera: Camera::of(Some(camera.get()), block_editor_beui::beui::Pos2::ZERO),
            palette: palette(&theme),
            title: label
                .as_ref()
                .map(|label| label.name.clone())
                .unwrap_or_else(|| match loading {
                    true => "Loading…".to_owned(),
                    false => "Broken link".to_owned(),
                }),
            glyph: label
                .as_ref()
                .and_then(|label| label.icon)
                .map(str::to_owned),
            automatic: label.is_some_and(|label| label.automatic),
            placeholder: placeholder.get(),
            entity,
        };
        if shown.borrow().as_ref() == Some(&paint) {
            return;
        }
        *shown.borrow_mut() = Some(paint.clone());
        set_draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| {
            paint.draw(&painter.in_document())
        }));
    });
    view! {
        <CanvasItem
            x={create_memo(move || bounds.get().left())}
            y={create_memo(move || bounds.get().top())}
            width={create_memo(move || bounds.get().width().max(1.0))}
            height={create_memo(move || bounds.get().height().max(1.0))}
            @test_id={format!("infinite-canvas.entity.{id}")}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn EntityEmbed(state: CanvasState, id: Uuid) -> CanvasItem {
    let entity = entity_of(state, id);
    let bounds = create_memo(move || entity.get().map(content_bounds).unwrap_or(Rect::ZERO));
    let target = create_memo(move || {
        let entity = entity.get().filter(|entity| !entity.style.hidden)?;
        let id = reference_of(&entity)?;
        let block_type = state.block_type_of(id)?;
        Some(ChildTarget::new(id, block_type))
    });
    let rotation =
        create_memo(move || entity.get().map_or(0.0, |entity| entity.transform.rotation));
    let opacity = create_memo(move || {
        entity
            .get()
            .map_or(1.0, |entity| entity.style.opacity.clamp(0.0, 1.0))
    });
    let mode = create_memo(move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::DirectEditor { .. } => match state.focused_editor.get() == Some(id) {
                true => ChildMode::Active,
                false => match (state.shows_preview(id), state.works_in_place(id)) {
                    (true, _) => ChildMode::Preview,
                    (false, true) => ChildMode::InPlace,
                    (false, false) => ChildMode::Passive,
                },
            },
            _ => ChildMode::Preview,
        },
        None => ChildMode::Preview,
    });
    let intrinsic = create_memo(move || {
        entity
            .get()
            .and_then(|entity| state.assigned_intrinsic(&entity))
    });
    let punch = create_memo(move || state.child_state(id).available);
    let editor = state.editor();
    view! {
        <CanvasItem
            x={create_memo(move || bounds.get().left())}
            y={create_memo(move || bounds.get().top())}
            width={create_memo(move || bounds.get().width().max(1.0))}
            height={create_memo(move || bounds.get().height().max(1.0))}
        >
            <ChildBlock
                editor={editor}
                block={target}
                mode={mode}
                punch={punch}
                rotation={rotation}
                opacity={opacity}
                intrinsic={intrinsic}
                on_state={move |child: ChildState| state.report_child(id, child)}
                on_view_change={move |change: ViewChange| state.apply_view_change(id, change)}
            />
        </CanvasItem>
    }
}

fn entity_of(state: CanvasState, id: Uuid) -> Memo<Option<CanvasEntity>> {
    create_memo(move || state.displayed().into_iter().find(|entity| entity.id == id))
}

fn palette(theme: &block_editor_beui::beui::styled::ThemeStore) -> Palette {
    Palette {
        auto: theme.text.get(),
        surface: theme.surface_raised.get(),
        border: theme.border.get(),
        muted: theme.text_muted.get(),
    }
}

fn content_bounds(entity: CanvasEntity) -> Rect {
    match entity.kind {
        CanvasEntityKind::Block { .. } => {
            let size = Vec2::new(
                entity.transform.size.x.max(MIN_SIZE),
                entity.transform.size.y.max(MIN_SIZE),
            );
            Rect::from_min_max(
                pos2(
                    entity.transform.center.x - size.x / 2.0,
                    entity.transform.center.y - size.y / 2.0,
                ),
                pos2(
                    entity.transform.center.x + size.x / 2.0,
                    entity.transform.center.y + size.y / 2.0,
                ),
            )
        }
        CanvasEntityKind::DirectEditor { .. } => direct_editor_layout(&entity)
            .map(|layout| layout.content.rect())
            .unwrap_or(Rect::ZERO),
        _ => Rect::ZERO,
    }
}

fn reference_of(entity: &CanvasEntity) -> Option<Uuid> {
    match entity.kind {
        CanvasEntityKind::Block { block_id } | CanvasEntityKind::DirectEditor { block_id, .. } => {
            Some(block_id)
        }
        _ => None,
    }
}

fn item_bounds(entity: &CanvasEntity) -> Rect {
    let bounds = entity_bounds(entity);
    let margin = entity.style.line_width.max(0.0) + ITEM_MARGIN;
    Rect::from_min_max(
        pos2(bounds.min.x - margin, bounds.min.y - margin),
        pos2(bounds.max.x + margin, bounds.max.y + margin),
    )
}
