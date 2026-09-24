use std::rc::Rc;

use block_client::blocks::infinite_canvas::{CanvasEntity, CanvasEntityKind};
use std::cell::RefCell;

use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, CanvasView, Child, ClickCatcher, Draw, Drawing, Focusable, ForEach, Memo,
    ReadSignal, Show, clone, component, component_rect, create_effect, create_memo, create_signal,
    view,
};
use block_editor_plugin::beui::styled::use_theme;
use block_editor_plugin::beui::{KeyPress, NodeId, PointerPress, Rect, Vec2, pos2};
use block_editor_plugin::{ChildBlock, ChildMode, ChildState, ChildTarget, ViewChange};
use uuid::Uuid;

use crate::geometry::*;

use super::menu::CanvasMenu;
use super::overlay::Overlay;
use super::paint::{Camera, EntityPaint, Palette};
use super::state::CanvasState;

const ITEM_MARGIN: f32 = 2.0;

#[component]
pub(crate) fn CanvasStage(state: Rc<CanvasState>) -> NodeId {
    let placed = component_rect();
    let held = Rc::clone(&state);
    create_effect(clone!(placed held -> move || held.stage.set(placed.get())));

    let view = state.editor().canvas();
    let world = state.editor().world();
    let centred = Rc::clone(&state);
    let camera = create_memo(clone!(view world placed centred -> move || {
        super::state::camera_view(view.get(), world.get(), placed.get(), centred.centre())
    }));
    let ids = create_memo(clone!(state -> move || {
        state
            .displayed()
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<Uuid>>()
    }));
    let embeds = ids.clone();
    let surface = Rc::clone(&state);
    let shapes = Rc::clone(&state);
    let children = Rc::clone(&state);
    let over = Rc::clone(&state);
    let shape_camera = camera.clone();
    let overlay_camera = camera.clone();
    let overlay_stage = placed.clone();
    let placement = camera.clone();
    let menu = Rc::clone(&state);
    let previewing = state.previewing();
    view! {
        <CanvasMenu state={menu} disabled={previewing}>
            <CanvasSurface state={surface}>
                <Canvas
                    view={create_memo(clone!(placement -> move || Some(placement.get())))}
                    @test_id={"infinite-canvas.canvas"}
                >
                    <ForEach keys={ids}>
                        {move |id: Uuid| {
                            let state = Rc::clone(&shapes);
                            let camera = shape_camera.clone();
                            view! {
                                <EntityShape state id camera />
                            }
                        }}
                    </ForEach>
                    <ForEach keys={embeds}>
                        {move |id: Uuid| {
                            let state = Rc::clone(&children);
                            view! {
                                <EntityEmbed state id />
                            }
                        }}
                    </ForEach>
                    <Show condition={!previewing}>
                        <CanvasOverlay state={over} camera={overlay_camera} stage={overlay_stage} />
                    </Show>
                </Canvas>
            </CanvasSurface>
        </CanvasMenu>
    }
}

#[component]
fn CanvasSurface(state: Rc<CanvasState>, children: Option<Child>) -> NodeId {
    let pressing = Rc::clone(&state);
    let dragging = Rc::clone(&state);
    let releasing = Rc::clone(&state);
    let hovering = Rc::clone(&state);
    let leaving = Rc::clone(&state);
    let secondary = Rc::clone(&state);
    let keying = Rc::clone(&state);
    let texting = Rc::clone(&state);
    let cursor = create_memo(clone!(state -> move || state.cursor()));
    let interactive = !state.previewing();
    view! {
        <Focusable
            tab_stop={interactive}
            on_key={move |press: KeyPress| keying.key(press)}
            on_text={move |text: String| {
                texting.paste_text(&text);
            }}
        >
            <ClickCatcher
                cursor={cursor}
                on_press={move |press: PointerPress| pressing.press(press)}
                on_secondary_press={move |press: PointerPress| secondary.secondary_press(press)}
                on_drag={move |press: PointerPress| dragging.drag(press)}
                on_hover_move={move |press: PointerPress| {
                    let at = hovering.world_at(press.pos);
                    hovering.hover(Some(at));
                }}
                on_hover_change={move |over: bool| {
                    if !over {
                        leaving.hover(None);
                    }
                }}
                on_active_change={move |active: bool| {
                    if !active {
                        releasing.release();
                    }
                }}
                children={children}
            />
        </Focusable>
    }
}

#[component]
fn CanvasOverlay(
    state: Rc<CanvasState>,
    camera: Memo<CanvasView>,
    stage: ReadSignal<Rect>,
) -> CanvasItem {
    let visible =
        create_memo(clone!(camera stage -> move || camera.get().rect_to_canvas(stage.get())));
    let theme = use_theme();
    let drawn = Rc::clone(&state);
    let (draw, set_draw) = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<Overlay>);
    create_effect(clone!(drawn theme camera stage -> move || {
        let overlay = Overlay {
            camera: Camera::of(Some(camera.get()), stage.get().min),
            palette: palette(&theme),
            stage: stage.get(),
            empty: drawn.entities.get().is_empty(),
            region: drawn.preview_region.get(),
            gesture: drawn.gesture.get(),
            frame: super::state::selection_frame(&drawn.displayed(), &drawn.selection.get()),
            resize: drawn.selection_handles().0,
            rotatable: drawn.selection_has_unlocked() && drawn.selection_allows_rotation(),
            presence: drawn.presence.get(),
            tool: drawn.tool.get(),
            pointer: drawn.pointer.get(),
        };
        if shown.borrow().as_ref() == Some(&overlay) {
            return;
        }
        *shown.borrow_mut() = Some(overlay.clone());
        set_draw.set_unconditionally(Rc::new(move |painter, _rect| overlay.draw(painter)));
    }));
    let x = create_memo(clone!(visible -> move || visible.get().left()));
    let y = create_memo(clone!(visible -> move || visible.get().top()));
    let width = create_memo(clone!(visible -> move || visible.get().width().max(1.0)));
    let height = create_memo(clone!(visible -> move || visible.get().height().max(1.0)));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn EntityShape(state: Rc<CanvasState>, id: Uuid, camera: Memo<CanvasView>) -> CanvasItem {
    let entity = entity_of(&state, id);
    let bounds = create_memo(clone!(entity -> move || {
        entity
            .get()
            .map(|entity| item_bounds(&entity))
            .unwrap_or(Rect::ZERO)
    }));
    let theme = use_theme();
    let drawn = Rc::clone(&state);
    let (draw, set_draw) = create_signal::<Draw>(Rc::new(|_, _| {}));
    let shown = RefCell::new(None::<EntityPaint>);
    create_effect(clone!(entity drawn theme camera -> move || {
        let Some(entity) = entity.get() else {
            return;
        };
        let reference = reference_of(&entity);
        let label = reference.and_then(|reference| drawn.label_of(reference));
        let loading = reference.is_some();
        let paint = EntityPaint {
            covered: drawn.child_state(entity.id).available,
            camera: Camera::of(Some(camera.get()), block_editor_plugin::beui::Pos2::ZERO),
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
            measure: drawn.measure_text(entity.id),
            entity,
        };
        if shown.borrow().as_ref() == Some(&paint) {
            return;
        }
        *shown.borrow_mut() = Some(paint.clone());
        set_draw.set_unconditionally(Rc::new(move |painter, _rect| paint.draw(painter)));
    }));
    view! {
        <CanvasItem
            x={create_memo(clone!(bounds -> move || bounds.get().left()))}
            y={create_memo(clone!(bounds -> move || bounds.get().top()))}
            width={create_memo(clone!(bounds -> move || bounds.get().width().max(1.0)))}
            height={create_memo(clone!(bounds -> move || bounds.get().height().max(1.0)))}
            @test_id={format!("infinite-canvas.entity.{id}")}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

#[component]
fn EntityEmbed(state: Rc<CanvasState>, id: Uuid) -> CanvasItem {
    let entity = entity_of(&state, id);
    let bounds = create_memo(clone!(entity -> move || {
        entity.get().map(content_bounds).unwrap_or(Rect::ZERO)
    }));
    let resolving = Rc::clone(&state);
    let target = create_memo(clone!(entity resolving -> move || {
        let entity = entity.get()?;
        let id = reference_of(&entity)?;
        let block_type = resolving.block_type_of(id)?;
        Some(ChildTarget::new(id, block_type))
    }));
    let rotation = create_memo(clone!(entity -> move || {
        entity.get().map_or(0.0, |entity| entity.transform.rotation)
    }));
    let opacity = create_memo(clone!(entity -> move || {
        entity
            .get()
            .map_or(1.0, |entity| entity.style.opacity.clamp(0.0, 1.0))
    }));
    let moded = Rc::clone(&state);
    let mode = create_memo(clone!(entity moded -> move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::DirectEditor { .. } => {
                match moded.focused_editor.get() == Some(id) {
                    true => ChildMode::Active,
                    false => match moded.shows_preview(id) {
                        true => ChildMode::Preview,
                        false => ChildMode::Passive,
                    },
                }
            }
            _ => ChildMode::Preview,
        },
        None => ChildMode::Preview,
    }));
    let sizing = Rc::clone(&state);
    let intrinsic = create_memo(clone!(entity sizing -> move || {
        entity
            .get()
            .and_then(|entity| sizing.assigned_intrinsic(&entity))
    }));
    let punching = Rc::clone(&state);
    let punch = create_memo(clone!(punching -> move || punching.child_state(id).available));
    let reporting = Rc::clone(&state);
    let changing = Rc::clone(&state);
    let editor = state.editor().clone();
    view! {
        <CanvasItem
            x={create_memo(clone!(bounds -> move || bounds.get().left()))}
            y={create_memo(clone!(bounds -> move || bounds.get().top()))}
            width={create_memo(clone!(bounds -> move || bounds.get().width().max(1.0)))}
            height={create_memo(clone!(bounds -> move || bounds.get().height().max(1.0)))}
        >
            <ChildBlock
                editor={editor}
                block={target}
                mode={mode}
                punch={punch}
                rotation={rotation}
                opacity={opacity}
                intrinsic={intrinsic}
                on_state={move |child: ChildState| reporting.report_child(id, child)}
                on_view_change={move |change: ViewChange| changing.apply_view_change(id, change)}
            />
        </CanvasItem>
    }
}

fn entity_of(state: &Rc<CanvasState>, id: Uuid) -> Memo<Option<CanvasEntity>> {
    let held = Rc::clone(state);
    create_memo(move || held.displayed().into_iter().find(|entity| entity.id == id))
}

fn palette(theme: &block_editor_plugin::beui::styled::ThemeStore) -> Palette {
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
