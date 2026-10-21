use std::rc::Rc;

use block_editor_beui::be_block::canvas::{CanvasEntity, CanvasPoint, CanvasShape};
use std::cell::RefCell;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{KeyPress, NodeId, Painter, PointerPress, Pos2, Rect, Vec2, pos2};
use block_editor_beui::{ChildBlock, ChildMode, ChildState, ChildTarget};
use uuid::Uuid;

use crate::geometry::*;

use super::grid::{GridLook, GridTile, GridTileItem};
use super::menu::CanvasMenu;
use super::overlay::{Backdrop, FrameBackdrop, Overlay};
use super::paint::{Camera, EntityPaint, Palette, arrowhead_size, path_middle, resolve_color};
use super::sketch::{Sketch, SketchKey, rough_reach};
use super::state::CanvasState;
use super::text::{EntityText, shown_live, text_box};

const ITEM_MARGIN: f32 = 2.0;

#[component]
pub(crate) fn CanvasStage(state: CanvasState) -> NodeId {
    view! {
        <PlacementBuilder>|placed: ReadSignal<Rect>| {
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
            let ids = state.entities.shown_ids();
            let inset = create_memo(move || {
                let (placed, shown) = (placed.get(), shown.get());
                Sides::new(
                    (shown.left() - placed.left()).max(0.0),
                    (shown.top() - placed.top()).max(0.0),
                    (placed.right() - shown.right()).max(0.0),
                    (placed.bottom() - shown.bottom()).max(0.0),
                )
            });
            let previewing = state.previewing();
            let (grid, tiles) = grid_of(state, camera, shown);
            view! {
                <CanvasMenu disabled={previewing}>
                    <Frame padding={inset}>
                        <CanvasSurface state={state}>
                            <Canvas
                                view={create_memo(move || Some(camera.get()))}
                                @test_id={"infinite-canvas.canvas"}
                            >
                                <CanvasBackdrop state={state} camera={camera} stage={shown} />
                                <ForEach keys={tiles}>|tile: GridTile|
                                    <GridTileItem tile look={grid} />
                                </ForEach>
                                <ForEach keys={ids}>|id: Uuid| {
                                    let camera = camera;
                                    view! {
                                        <EntityShape state id camera />
                                    }
                                }</ForEach>
                                <ForEach keys={ids}>|id: Uuid|
                                    <EntityEmbed state id />
                                </ForEach>
                                <Show condition={!previewing}>
                                    <CanvasOverlay state={state} camera={camera} stage={shown} />
                                </Show>
                            </Canvas>
                        </CanvasSurface>
                    </Frame>
                </CanvasMenu>
            }
        }</PlacementBuilder>
    }
}

fn grid_of(
    state: CanvasState,
    camera: Memo<CanvasView>,
    stage: Memo<Rect>,
) -> (Memo<Option<GridLook>>, Memo<Vec<GridTile>>) {
    let grid = state.grid();
    let live = !state.previewing();
    let theme = use_theme();
    let look = create_memo(move || {
        let spacing = grid.get().shown().filter(|_| live)?;
        GridLook::new(spacing, camera.get().scale, theme.border.get())
    });
    let tiles = create_memo(move || match look.get() {
        Some(look) => look.tiles(camera.get().rect_to_canvas(stage.get())),
        None => Vec::new(),
    });
    (look, tiles)
}

#[component]
fn CanvasSurface(state: CanvasState, children: Option<Child>) -> NodeId {
    let cursor = create_memo(move || state.cursor());
    let interactive = !state.previewing();
    let surface = state.surface();
    view! {
        <Interactive
            @node_ref=&surface
            focusable=true
            tab_stop={interactive}
            on_key={move |press: KeyPress| state.key(press)}
            pastes={vec![PasteKind::Image, PasteKind::Text]}
            on_paste={move |pasted: Pasted| state.take_paste(pasted)}
            cursor={cursor}
            touch_drags={interactive}
            drawing={interactive}
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
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<Overlay>);
    create_effect(move || {
        let selection = state.selection.get();
        let selected = state.entities.shown_among(&selection);
        let overlay = Overlay {
            camera: Camera::of(Some(camera.get()), stage.get().min),
            palette: palette(&theme),
            stage: stage.get(),
            empty: state.entities.ids().with(Vec::is_empty),
            gesture: state.gesture.get(),
            outlines: super::state::selected_outlines(&selected, &selection),
            frame: super::state::selection_frame(&selected, &selection),
            line: state
                .selected_path()
                .and_then(|path| Some(path_ends(&state.entities.shown(path.id)?))),
            resize: state.selection_handles().0,
            rotatable: state.selection_has_unlocked() && state.selection_allows_rotation(),
            presence: state.presence.get(),
            tool: state.tool.get(),
            pointer: state.pointer.get(),
            pen_width: state.default_style().stroke_width,
        };
        if shown.borrow().as_ref() == Some(&overlay) {
            return;
        }
        *shown.borrow_mut() = Some(overlay.clone());
        draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| {
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
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<Backdrop>);
    create_effect(move || {
        let frames = state
            .entities
            .shown_frames()
            .into_iter()
            .filter(|entity| !entity.hidden)
            .map(|entity| FrameBackdrop {
                bounds: entity_bounds(&entity),
                name: entity.text.clone(),
                fill: resolve_color(
                    entity.style.filled().unwrap_or_default(),
                    theme.background.get(),
                ),
            })
            .collect();
        let backdrop = Backdrop {
            camera: Camera::of(Some(camera.get()), stage.get().min),
            stage: stage.get(),
            label: theme.text_muted.get(),
            chrome: !state.previewing(),
            frames,
        };
        if shown.borrow().as_ref() == Some(&backdrop) {
            return;
        }
        *shown.borrow_mut() = Some(backdrop.clone());
        draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| {
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
    let editing = create_memo(move || state.editing_text.get() == Some(id));
    let live = create_memo(move || entity.get().is_some_and(|entity| shown_live(&entity)));
    let bounds = create_memo(move || {
        let Some(entity) = entity.get() else {
            return Rect::ZERO;
        };
        match live.get() {
            true => item_bounds(&entity).union(text_box(&entity)),
            false => item_bounds(&entity),
        }
    });
    let scales = create_memo(move || !live.get());
    let thumbhash = create_memo(move || state.thumbhash_of(reference_of(&entity.get()?)?));
    let placeholder = create_memo(move || thumbhash.get().and_then(|thumbhash| thumbhash.decode()));
    let theme = use_theme();
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<EntityPaint>);
    let sketched = RefCell::new(None::<(SketchKey, Sketch, Option<(Pos2, Rc<Sketch>)>)>);
    create_effect(move || {
        let Some(entity) = entity.get() else {
            return;
        };
        let corner = bounds.get().min;
        let key = SketchKey::of(&entity);
        let mut held = sketched.borrow_mut();
        if held.as_ref().is_none_or(|(held_key, ..)| *held_key != key) {
            *held = Some((key, Sketch::of(&entity), None));
        }
        let Some((_, world, placed)) = held.as_mut() else {
            return;
        };
        let sketch = match placed {
            Some((at, sketch)) if *at == corner => Rc::clone(sketch),
            _ => {
                let sketch = Rc::new(world.translated(Vec2::new(-corner.x, -corner.y)));
                *placed = Some((corner, Rc::clone(&sketch)));
                sketch
            }
        };
        drop(held);
        let reference = reference_of(&entity);
        let label = reference.and_then(|reference| state.label_of(reference));
        let missing = match reference {
            Some(_) => state.unlabeled(),
            None => "Broken link".to_owned(),
        };
        let paint = EntityPaint {
            sketch,
            covered: state.child_state(entity.id).available,
            camera: Camera::item(corner),
            palette: palette(&theme),
            title: label
                .as_ref()
                .map(|label| label.name.clone())
                .unwrap_or(missing),
            glyph: label
                .as_ref()
                .and_then(block_editor_beui::label_glyph)
                .map(str::to_owned),
            automatic: label.is_some_and(|label| label.automatic),
            placeholder: placeholder.get(),
            entity,
        };
        if shown.borrow().as_ref() == Some(&paint) {
            return;
        }
        *shown.borrow_mut() = Some(paint.clone());
        draw.set_unconditionally(Rc::new(move |painter: &Painter, _rect| paint.draw(painter)));
    });
    view! {
        <CanvasItem
            x={create_memo(move || bounds.get().left())}
            y={create_memo(move || bounds.get().top())}
            width={create_memo(move || bounds.get().width().max(1.0))}
            height={create_memo(move || bounds.get().height().max(1.0))}
            scales={scales}
            pointer={editing}
            @test_id={format!("infinite-canvas.entity.{id}")}
        >
            <Layers>
                <Drawing draw={draw} />
                <Show condition={live}>
                    <EntityText state id entity outer={bounds} camera editing />
                </Show>
            </Layers>
        </CanvasItem>
    }
}

#[component]
fn EntityEmbed(state: CanvasState, id: Uuid) -> CanvasItem {
    let entity = entity_of(state, id);
    let bounds = create_memo(move || entity.get().map(content_bounds).unwrap_or(Rect::ZERO));
    let target = create_memo(move || {
        let entity = entity.get().filter(|entity| !entity.hidden)?;
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
        Some(entity) => match entity.shape {
            CanvasShape::Editor => match state.focused_editor.get() == Some(id) {
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
    let linked = create_memo(move || entity.get().as_ref().and_then(reference_of).is_some());
    let shown = create_memo(move || match linked.get() {
        true => bounds.get(),
        false => Rect::ZERO,
    });
    let least = create_memo(move || match linked.get() {
        true => 1.0,
        false => 0.0,
    });
    view! {
        <CanvasItem
            x={create_memo(move || shown.get().left())}
            y={create_memo(move || shown.get().top())}
            width={create_memo(move || shown.get().width().max(least.get()))}
            height={create_memo(move || shown.get().height().max(least.get()))}
        >
            <Layers>
                <Show condition={linked}>
                    <ChildBlock
                        editor={editor}
                        block={target}
                        mode={mode}
                        punch={punch}
                        rotation={rotation}
                        opacity={opacity}
                        intrinsic={intrinsic}
                        on_state={move |child: ChildState| state.report_child(id, child)}
                    />
                </Show>
            </Layers>
        </CanvasItem>
    }
}

fn entity_of(state: CanvasState, id: Uuid) -> Memo<Option<CanvasEntity>> {
    state.entities.watch(id).shown()
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
    match entity.shape {
        CanvasShape::Block | CanvasShape::Image => {
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
        CanvasShape::Editor => direct_editor_layout(&entity)
            .map(|layout| layout.content.rect())
            .unwrap_or(Rect::ZERO),
        _ => Rect::ZERO,
    }
}

fn reference_of(entity: &CanvasEntity) -> Option<Uuid> {
    entity.block.filter(|_| entity.shape.embeds_block())
}

fn item_bounds(entity: &CanvasEntity) -> Rect {
    let bounds = entity_bounds(entity);
    let stroke = entity.style.stroke_width.max(0.0);
    let margin = ITEM_MARGIN
        + match entity.shape {
            CanvasShape::Freehand => stroke * MOST_PRESSED_WIDTH,
            CanvasShape::Path => stroke.max(arrowhead_size(stroke) * 0.5),
            _ => stroke,
        }
        + rough_reach(entity);
    let bounds = match entity.text.is_empty() || !entity.shape.has_points() {
        true => bounds,
        false => bounds.union(path_label_reach(entity)),
    };
    Rect::from_min_max(
        pos2(bounds.min.x - margin, bounds.min.y - margin),
        pos2(bounds.max.x + margin, bounds.max.y + margin),
    )
}

fn path_label_reach(entity: &CanvasEntity) -> WorldRect {
    let px = entity.style.text_size.px();
    let lines = entity.text.lines().count().max(1) as f32;
    let widest = entity
        .text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32;
    let half = CanvasPoint::new(widest * px * 0.35 + px, lines * px * 0.75 + px);
    let center = path_middle(&flattened(&entity.world_points()));
    WorldRect::from_points(
        CanvasPoint::new(center.x - half.x, center.y - half.y),
        CanvasPoint::new(center.x + half.x, center.y + half.y),
    )
}
