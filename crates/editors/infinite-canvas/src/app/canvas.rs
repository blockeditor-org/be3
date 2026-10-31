use block_editor_beui::beui::CanvasView;
use std::rc::Rc;

use infinite_canvas_types::{CanvasColor, CanvasEntity, CanvasShape};
use std::cell::RefCell;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    KeyPress, NodeId, PointerPress, Pos2, Rect, Vec2, pos2, reactive::Painter,
};
use block_editor_beui::{ChildBlock, ChildMode, ChildState, ChildTarget};
use uuid::Uuid;

use crate::geometry::*;

use super::grid::{GridLook, GridTile, GridTileItem};
use super::menu::CanvasMenu;
use super::overlay::{Badge, FrameBackdrop, Hint, Live, Overlay, PathGrips};
use super::paint::{Camera, EntityPaint, Palette, arrowhead_size, resolve_color};
use super::raster::{RasterGuideDrawing, raster_image, rastering};
use super::sketch::{Sketch, SketchKey, rough_reach};
use super::state::{CanvasState, Gesture};
use super::text::{EntityText, path_label_reach, text_region, writes};
use crate::freehand::LiveStroke;
use beui::{PasteKind, Pasted, Sides};

const ITEM_MARGIN: f32 = 2.0;

#[component]
pub(crate) fn CanvasStage(state: CanvasState) -> NodeId {
    let placed_held = create_signal(Rect::ZERO);
    let placed = placed_held.read_only();
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
    let ids = state.entities.ids();
    let embedding = create_memo(move || state.entities.embedding());
    let inset = create_memo(move || {
        let (placed, shown) = (placed.get(), shown.get());
        Padding::custom(Sides::new(
            (shown.left() - placed.left()).max(0.0),
            (shown.top() - placed.top()).max(0.0),
            (placed.right() - shown.right()).max(0.0),
            (placed.bottom() - shown.bottom()).max(0.0),
        ))
    });
    let previewing = state.previewing();
    let (grid, tiles) = grid_of(state, camera, shown);
    let frames = create_memo(move || {
        state
            .entities
            .frames()
            .iter()
            .filter(|frame| !frame.hidden)
            .map(|frame| entity_bounds(frame).rect())
            .collect::<Vec<_>>()
    });
    let frame_ids = create_memo(move || {
        state
            .entities
            .frames()
            .iter()
            .filter(|frame| !frame.hidden)
            .map(|frame| frame.id)
            .collect::<Vec<_>>()
    });
    let outside_frames =
        create_memo(
            move || match !previewing && frames.with(|frames| !frames.is_empty()) {
                true => ColorRole::Dim,
                false => ColorRole::None,
            },
        );
    let built = view! {
        <CanvasMenu disabled={previewing}>
            <Frame padding={inset} fill={outside_frames}>
                <CanvasSurface state={state}>
                    <Canvas
                        view={create_memo(move || Some(camera.get()))}
                        on_reveal={reveal_by(state.editor())}
                        @test_id={"infinite-canvas.canvas"}
                    >
                        <ForEach keys={frame_ids}>|id: Uuid| {
                            let camera = camera;
                            view! {
                                <FrameBackdropItem state id camera />
                            }
                        }</ForEach>
                        <ForEach keys={tiles}>|tile: GridTile|
                            <GridTileItem tile look={grid} frames />
                        </ForEach>
                        <ForEach keys={ids}>|id: Uuid|
                            <EntityShape state id />
                        </ForEach>
                        <ForEach keys={embedding}>|id: Uuid|
                            <EntityEmbed state id />
                        </ForEach>
                        <Show condition={!previewing}>
                            <CanvasHint state={state} camera={camera} stage={shown} />
                        </Show>
                        <Show condition={!previewing}>
                            <CanvasOverlay state={state} camera={camera} />
                        </Show>
                        <Show condition={!previewing}>
                            <LiveOverlay state={state} camera={camera} stage={shown} />
                        </Show>
                    </Canvas>
                </CanvasSurface>
            </Frame>
        </CanvasMenu>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed_held.set(measured.rect);
        }
    });
    built
}

const REVEAL_STEP: f32 = 0.35;

fn reveal_by(editor: block_editor_beui::Editor) -> Callback<Vec2> {
    Callback::new(move |delta: Vec2| {
        let step = match motion().animates() && (delta * REVEAL_STEP).length() >= 1.0 {
            true => delta * REVEAL_STEP,
            false => delta,
        };
        editor.pan(step);
    })
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
        GridLook::new(
            spacing,
            camera.get().scale,
            beui::reactive::resolve_color(theme.border.get()),
        )
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
    let inking = create_memo(move || {
        state
            .gesture
            .with(|gesture| matches!(gesture, Some(Gesture::Pen { .. })))
    });
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
            every_sample={inking}
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
fn CanvasHint(
    state: CanvasState,
    camera: Memo<CanvasView>,
    stage: Memo<Rect>,
) -> DynamicSegment<CanvasItem> {
    let theme = use_theme();
    let hint = create_memo(move || {
        let empty = state.entities.ids().with(Vec::is_empty);
        let busy = state.gesture.with(Option::is_some);
        (empty && !busy).then(|| Hint {
            color: beui::reactive::resolve_color(theme.text_muted.get()),
        })
    });
    let shown = create_memo(move || hint.with(Option::is_some));
    let visible = create_memo(move || camera.get().rect_to_canvas(stage.get()));
    let draw = move || {
        Prop::dynamic(move || -> Draw {
            let hint = hint.get();
            Rc::new(move |painter: &Painter, rect: Rect| {
                if let Some(hint) = hint.as_ref() {
                    hint.draw(painter, rect);
                }
            })
        })
    };
    view! {
        <Show condition={shown}>
            <CanvasItem
                x={create_memo(move || visible.get().left())}
                y={create_memo(move || visible.get().top())}
                width={create_memo(move || visible.get().width().max(1.0))}
                height={create_memo(move || visible.get().height().max(1.0))}
            >
                <Drawing draw={draw()} />
            </CanvasItem>
        </Show>
    }
}

#[component]
fn CanvasOverlay(state: CanvasState, camera: Memo<CanvasView>) -> DynamicSegment<CanvasItem> {
    let theme = use_theme();
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let bounds = create_signal(None::<Rect>);
    let shown = RefCell::new(None::<Overlay>);
    let scale = create_memo(move || camera.get().scale.max(f32::EPSILON));
    let drawn_line = create_memo(move || {
        state.gesture.with(|gesture| match gesture {
            Some(Gesture::Create { id, tool, .. }) if tool.draws_a_line() => Some(*id),
            _ => None,
        })
    });
    create_effect(move || {
        let selection = state.selection.get();
        let selected = state.entities.among(&selection);
        let scale = scale.get();
        let selected_path = state.selected_path();
        let mut overlay = Overlay {
            camera: Camera::item(Pos2::ZERO, scale),
            palette: palette(&theme),
            outlines: super::state::selected_outlines(&selected, &selection),
            frame: super::state::selection_frame(&selected, &selection),
            path: selected_path.as_ref().map(|path| path_grips(path, scale)),
            anchors: drawn_line
                .get()
                .and_then(|id| state.entities.get(id))
                .or_else(|| selected_path.clone())
                .map(|path| state.anchors(&path))
                .unwrap_or_default(),
            group: entered_group_bounds(state),
            resize: state.selection_handles().0,
            rotatable: state.selection_has_unlocked() && state.selection_allows_rotation(),
            selections: state.presence.with(|presence| presence.selections.clone()),
        };
        let reach = overlay.reach().map(|reach| {
            let margin = Overlay::margin(scale);
            let whole = |at: f32| (at * scale).floor() / scale;
            Rect::from_min_max(
                pos2(whole(reach.min.x - margin), whole(reach.min.y - margin)),
                pos2(reach.max.x + margin, reach.max.y + margin),
            )
        });
        if bounds.get_untracked() != reach {
            bounds.set(reach);
        }
        overlay.camera = Camera::item(reach.map_or(Pos2::ZERO, |reach| reach.min), scale);
        if shown.borrow().as_ref() == Some(&overlay) {
            return;
        }
        *shown.borrow_mut() = Some(overlay.clone());
        draw.set_unconditionally(Rc::new(move |painter: &Painter, rect: Rect| {
            overlay.draw(&painter.transformed(rect.min.to_vec2(), 1.0))
        }));
    });
    let present = create_memo(move || bounds.with(Option::is_some));
    let rect = create_memo(move || bounds.get().unwrap_or(Rect::ZERO));
    view! {
        <Show condition={present}>
            <CanvasItem
                x={create_memo(move || rect.get().left())}
                y={create_memo(move || rect.get().top())}
                width={create_memo(move || rect.get().width().max(f32::EPSILON))}
                height={create_memo(move || rect.get().height().max(f32::EPSILON))}
            >
                <Drawing draw={draw} />
            </CanvasItem>
        </Show>
    }
}

#[component]
fn LiveOverlay(
    state: CanvasState,
    camera: Memo<CanvasView>,
    stage: Memo<Rect>,
) -> DynamicSegment<CanvasItem> {
    let visible = create_memo(move || camera.get().rect_to_canvas(stage.get()));
    let theme = use_theme();
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let present = create_signal(false);
    let shown = RefCell::new(None::<Live>);
    create_effect(move || {
        let presence = state.presence.get();
        let tool = state.tool.get();
        let live = Live {
            camera: Camera::item(visible.get().min, camera.get().scale.max(f32::EPSILON)),
            palette: palette(&theme),
            gesture: state.gesture.get(),
            cursors: presence.cursors,
            pen: state.default_style(),
            badge: state
                .pointer
                .get()
                .filter(|_| !tool.selects())
                .map(|at| Badge {
                    glyph: tool.glyph(),
                    at,
                }),
        };
        if present.get_untracked() == live.is_empty() {
            present.set(!live.is_empty());
        }
        if shown.borrow().as_ref() == Some(&live) {
            return;
        }
        *shown.borrow_mut() = Some(live.clone());
        draw.set_unconditionally(Rc::new(move |painter: &Painter, rect: Rect| {
            live.draw(
                &painter.transformed(rect.min.to_vec2(), 1.0),
                Rect::from_min_size(Pos2::ZERO, rect.size()),
            )
        }));
    });
    view! {
        <Show condition={present}>
            <CanvasItem
                x={create_memo(move || visible.get().left())}
                y={create_memo(move || visible.get().top())}
                width={create_memo(move || visible.get().width().max(1.0))}
                height={create_memo(move || visible.get().height().max(1.0))}
            >
                <Drawing draw={draw} />
            </CanvasItem>
        </Show>
    }
}

#[component]
fn FrameBackdropItem(state: CanvasState, id: Uuid, camera: Memo<CanvasView>) -> CanvasItem {
    let entity = entity_of(state, id);
    let label = create_memo(move || FRAME_LABEL_HEIGHT / camera.get().scale.max(f32::EPSILON));
    let bounds = create_memo(move || {
        let Some(entity) = entity.get() else {
            return Rect::ZERO;
        };
        let frame = entity_bounds(&entity).rect();
        Rect::from_min_max(pos2(frame.left(), frame.top() - label.get()), frame.max)
    });
    let theme = use_theme();
    let chrome = !state.previewing();
    let backdrop = create_memo(move || {
        let entity = entity.get()?;
        Some(FrameBackdrop {
            name: entity.text.to_str_lossy().into_owned(),
            fill: match entity.style.filled().unwrap_or_default() {
                CanvasColor::Auto => beui::reactive::resolve_color(theme.background.get()),
                color => resolve_color(color, beui::reactive::resolve_color(theme.text.get())),
            },
            label: beui::reactive::resolve_color(theme.text_muted.get()),
            outline: beui::reactive::resolve_color(theme.border.get()),
            chrome,
        })
    });
    let draw = Prop::dynamic(move || -> Draw {
        let backdrop = backdrop.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            if let Some(backdrop) = backdrop.as_ref() {
                let frame = Rect::from_min_max(
                    pos2(rect.left(), rect.top() + FRAME_LABEL_HEIGHT),
                    rect.max,
                );
                backdrop.draw(painter, frame);
            }
        })
    });
    view! {
        <CanvasItem
            x={create_memo(move || bounds.get().left())}
            y={create_memo(move || bounds.get().top())}
            width={create_memo(move || bounds.get().width().max(1.0))}
            height={create_memo(move || bounds.get().height().max(1.0))}
            @test_id={format!("infinite-canvas.backdrop.{id}")}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn EntityShape(state: CanvasState, id: Uuid) -> CanvasItem {
    let entity = entity_of(state, id);
    let editing = create_memo(move || state.editing_text.get() == Some(id));
    let live = create_memo(move || {
        entity.get().is_some_and(|entity| {
            writes(&entity)
                && (entity.shape == CanvasShape::Text || !entity.text.is_empty() || editing.get())
        })
    });
    let bounds = create_memo(move || {
        let Some(entity) = entity.get() else {
            return Rect::ZERO;
        };
        match live.get() {
            true => item_bounds(&entity).union(text_region(&entity)),
            false => item_bounds(&entity),
        }
    });
    let drawing = create_memo(move || {
        let freehand = entity.with(|entity| {
            entity
                .as_ref()
                .is_some_and(|entity| entity.shape == CanvasShape::Freehand)
        });
        freehand && state.being_drawn(id)
    });
    let raster = state.entities.watch(id).raster();
    let image = create_memo(move || raster.with(|raster| raster.as_ref().map(raster_image)));
    let rastered = create_memo(move || raster.with(Option::is_some));
    let raster_profile = rastering(state);
    let guided = create_memo(move || !state.previewing() && raster_profile.get() && rastered.get());
    let corner = create_memo(move || bounds.get().min);
    let thumbhash = create_memo(move || state.thumbhash_of(reference_of(&entity.get()?)?));
    let placeholder = create_memo(move || thumbhash.get().and_then(|thumbhash| thumbhash.decode()));
    let theme = use_theme();
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<EntityPaint>);
    let sketched = RefCell::new(None::<(SketchKey, Sketch, Option<(Pos2, Rc<Sketch>)>)>);
    let inked = RefCell::new(None::<LiveStroke>);
    create_effect(move || {
        let Some(entity) = entity.get() else {
            return;
        };
        let corner = bounds.get().min;
        let drawing = drawing.get();
        let sketch = match drawing {
            true => {
                *sketched.borrow_mut() = None;
                let width = entity.style.stroke_width.max(0.1);
                let mut inked = inked.borrow_mut();
                let live = match inked.as_mut() {
                    Some(live) if live.size() == width => live,
                    _ => inked.insert(LiveStroke::new(width)),
                };
                let points: Vec<Pos2> = entity
                    .world_points()
                    .iter()
                    .map(|point| pos2(point.x, point.y))
                    .collect();
                let pieces = live.draw(&points, &entity.pressures);
                Rc::new(Sketch::painted(pieces).translated(Vec2::new(-corner.x, -corner.y)))
            }
            false => {
                *inked.borrow_mut() = None;
                let key = SketchKey::of(&entity, drawing);
                let mut held = sketched.borrow_mut();
                if held.as_ref().is_none_or(|(held_key, ..)| *held_key != key) {
                    *held = Some((key, Sketch::of(&entity, drawing), None));
                }
                let Some((_, world, placed)) = held.as_mut() else {
                    return;
                };
                match placed {
                    Some((at, sketch)) if *at == corner => Rc::clone(sketch),
                    _ => {
                        let sketch = Rc::new(world.translated(Vec2::new(-corner.x, -corner.y)));
                        *placed = Some((corner, Rc::clone(&sketch)));
                        sketch
                    }
                }
            }
        };
        let reference = reference_of(&entity);
        let label = reference.and_then(|reference| state.label_of(reference));
        let missing = match reference {
            Some(_) => state.unlabeled(),
            None => "Broken link".to_owned(),
        };
        let paint = EntityPaint {
            sketch,
            covered: state.child_state(entity.id).available,
            camera: Camera::item(corner, 1.0),
            palette: palette(&theme),
            title: label
                .as_ref()
                .map(|label| label.name.clone())
                .unwrap_or(missing),
            glyph: label.as_ref().and_then(block_editor_beui::label_glyph),
            automatic: label.is_some_and(|label| label.automatic),
            placeholder: placeholder.get(),
            raster: image.get(),
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
            scales=true
            pointer={editing}
            @test_id={format!("infinite-canvas.entity.{id}")}
        >
            <Layers>
                <Drawing draw={draw} />
                <Show condition={guided}>
                    <RasterGuideDrawing state entity raster corner />
                </Show>
                <Show condition={live}>
                    <EntityText state id entity outer={bounds} editing />
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

fn path_grips(path: &CanvasEntity, scale: f32) -> PathGrips {
    let world = path.world_points();
    PathGrips {
        outline: flattened(&world),
        points: world.iter().map(|point| point.at()).collect(),
        middles: path_middles(path, scale)
            .into_iter()
            .map(|(_, at)| at)
            .collect(),
    }
}

fn entered_group_bounds(state: CanvasState) -> Option<WorldRect> {
    let group = state.entered_group.get()?;
    let members = state
        .entities
        .all_untracked()
        .into_iter()
        .filter(|entity| entity.group_id == Some(group))
        .map(|entity| entity.id)
        .collect();
    state
        .entities
        .among(&members)
        .iter()
        .map(entity_bounds)
        .reduce(WorldRect::union)
}

fn entity_of(state: CanvasState, id: Uuid) -> Memo<Option<CanvasEntity>> {
    state.entities.watch(id).stored()
}

pub(crate) fn palette(theme: &block_editor_beui::beui::styled::ThemeStore) -> Palette {
    Palette {
        auto: beui::reactive::resolve_color(theme.text.get()),
        surface: beui::reactive::resolve_color(theme.surface_raised.get()),
        border: beui::reactive::resolve_color(theme.border.get()),
        muted: beui::reactive::resolve_color(theme.text_muted.get()),
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

pub(crate) fn item_bounds(entity: &CanvasEntity) -> Rect {
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
