use std::rc::Rc;

use block_editor_beui::be_block::map::MapPoint;
use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, Child, ClickCatcher, Focusable, ForEach, Frame, ItemSize, List, Memo,
    NodeRef, clone, component, create_effect, create_memo, create_selector, untrack, view,
};
use block_editor_beui::beui::styled::{Caption, use_theme};
use block_editor_beui::beui::{
    Color32, CursorIcon, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, TextAlign, Vec2,
};
use uuid::Uuid;

use crate::geo::MapView;
use crate::points::{self, Marker};
use crate::tiles::TileId;

use super::labels::{self, MapLabel};
use super::state::MapState;
use super::tiles::{self, Tile};

pub(crate) const REGION_COLOR: Color32 = Color32::from_rgb(245, 180, 60);
const ATTRIBUTION: &str = "© OpenStreetMap contributors";
const HALO: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
const ATTRIBUTION_HEIGHT: f32 = 16.0;

#[component]
pub(crate) fn MapCanvas(state: Rc<MapState>) -> NodeId {
    let placed = Rc::clone(&state);
    let revision = state.revision.clone();
    let sized = state.editor().world();
    let laid_out = state.editor().placed();
    let world = create_memo(clone!(placed sized laid_out -> move || {
        placed.displayed_region.with(|_| ());
        sized.with(|_| ());
        laid_out.with(|_| ());
        placed.anchor.with(|_| ());
        placed.view()
    }));
    let viewed = Rc::clone(&state);
    let camera = create_memo(move || viewed.camera());
    let scale =
        create_memo(clone!(camera -> move || camera.get().map_or(1.0, |camera| camera.scale)));
    let seen = Rc::clone(&state);
    let visible = create_memo(clone!(sized -> move || {
        sized.with(|_| ());
        seen.visible()
    }));
    let covered = Rc::clone(&state);
    let covering = create_memo(clone!(world visible scale -> move || {
        let (world, visible, scale) = (world.get(), visible.get(), scale.get());
        match covered.sized() {
            true => tiles::covering(world, visible, scale),
            false => Vec::new(),
        }
    }));
    let used = Rc::clone(&state);
    create_effect(clone!(covering -> move || {
        let shown = covering.get();
        untrack(|| used.use_tiles(&shown));
    }));

    let labelled = Rc::clone(&state);
    let measures = Rc::new(labels::Measures::default());
    let candidates = create_memo(clone!(covering world revision -> move || {
        let _ = revision.get();
        let held = labelled.tiles();
        let sources = labels::sources(&held, &covering.get());
        Rc::new(labels::candidates(&held, &sources, world.get(), &measures))
    }));
    let decluttered = create_memo(clone!(candidates scale -> move || {
        Rc::new(labels::declutter(&candidates.get(), scale.get()))
    }));
    let shown_labels = create_memo(clone!(decluttered visible scale -> move || {
        labels::shown(&decluttered.get(), visible.get(), scale.get())
    }));
    let label_keys = create_memo(clone!(shown_labels -> move || {
        shown_labels.with(|shown| shown.iter().map(labels::Label::key).collect::<Vec<_>>())
    }));
    let by_key = create_memo(
        clone!(shown_labels -> move || shown_labels.with(|shown| labels::by_key(shown))),
    );

    let region = Rc::clone(&state);
    let outline = create_memo(clone!(region world -> move || {
        let view = world.get();
        region
            .preview_region
            .get()
            .map(|preview| view.region_rect(preview))
    }));
    let has_region = create_memo(clone!(outline -> move || outline.get().is_some()));
    let region_rect = create_memo(clone!(outline -> move || outline.get().unwrap_or(Rect::ZERO)));

    let points = state.points.clone();
    let ids = create_memo(clone!(points -> move || {
        points.get().iter().map(|point| point.id).collect::<Vec<Uuid>>()
    }));
    let selection = create_selector(clone!(state -> move || state.selected.get()));

    let theme = use_theme();
    let markers = Rc::clone(&state);
    let interaction = Rc::clone(&state);
    let tile_scale = scale.clone();
    let marker_scale = scale.clone();
    let marker_world = world.clone();
    let underlay_scale = scale.clone();
    let underlay_world = world.clone();
    let underlaid = Rc::clone(&state);
    let content = NodeRef::new();
    state.editor().content(&content);

    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <MapSurface
                    state={interaction}
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                >
                    <Canvas view={camera} @test_id={"map.canvas"}>
                        <ForEach keys={covering.clone()}>
                            {move |tile: TileId| {
                                let state = Rc::clone(&underlaid);
                                let world = underlay_world.clone();
                                let scale = underlay_scale.clone();
                                view! {
                                    <Tile state tile world scale underlay=true />
                                }
                            }}
                        </ForEach>
                        <ForEach keys={covering}>
                            {move |tile: TileId| {
                                let state = Rc::clone(&placed);
                                let world = world.clone();
                                let scale = tile_scale.clone();
                                view! {
                                    <Tile state tile world scale />
                                }
                            }}
                        </ForEach>
                        <ForEach keys={label_keys}>
                            {move |key: labels::LabelKey| {
                                let shown = by_key.clone();
                                let scale = scale.clone();
                                view! {
                                    <MapLabel key shown scale />
                                }
                            }}
                        </ForEach>
                        <RegionOutline shown={has_region} rect={region_rect} />
                        <ForEach keys={ids}>
                            {move |id: Uuid| {
                                let state = Rc::clone(&markers);
                                let selected = selection.memo(Some(id));
                                let world = marker_world.clone();
                                let scale = marker_scale.clone();
                                view! {
                                    <PointMarker state id selected world scale />
                                }
                            }}
                        </ForEach>
                    </Canvas>
                </MapSurface>
                <Attribution />
            </List>
        </Frame>
    }
}

#[component]
fn RegionOutline(shown: Memo<bool>, rect: Memo<Rect>) -> CanvasItem {
    let x = create_memo(clone!(rect -> move || rect.get().left()));
    let y = create_memo(clone!(rect -> move || rect.get().top()));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame visible={shown} outline=REGION_COLOR outline_width=1.5 outline_visible=true />
        </CanvasItem>
    }
}

#[component]
fn PointMarker(
    state: Rc<MapState>,
    id: Uuid,
    selected: Memo<bool>,
    world: Memo<MapView>,
    scale: Memo<f32>,
) -> CanvasItem {
    let held = Rc::clone(&state);
    let point = create_memo(clone!(held -> move || {
        held.points.get().into_iter().find(|point| point.id == id)
    }));
    let tip = create_memo(clone!(point world -> move || {
        let view = world.get();
        point
            .get()
            .map(|point: MapPoint| view.position(point.position))
            .unwrap_or(Pos2::ZERO)
    }));
    let color = create_memo(clone!(point -> move || {
        point.get().map_or(Color32::TRANSPARENT, |point| points::marker_color(point.color))
    }));
    let named = Rc::clone(&state);
    let label = create_memo(clone!(point named -> move || {
        let Some(point) = point.get() else {
            return String::new();
        };
        match named.label_of(point.block_id) {
            Some(label) => label.name,
            None => "Loading…".to_owned(),
        }
    }));
    view! {
        <Marker tip={tip} scale={scale} color={color} selected={selected} label={label} />
    }
}

#[component]
fn Attribution() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame height=ATTRIBUTION_HEIGHT color=HALO padding_horizontal=6.0>
            <Caption content=ATTRIBUTION color={theme.text_muted.clone()} align=TextAlign::End />
        </Frame>
    }
}

#[component]
fn MapSurface(state: Rc<MapState>, children: Option<Child>) -> NodeId {
    let pressing = Rc::clone(&state);
    let dragging = Rc::clone(&state);
    let releasing = Rc::clone(&state);
    let pasting = Rc::clone(&state);
    let panning = Rc::clone(&state);
    view! {
        <Focusable
            on_key={move |press: KeyPress| {
                if press.key == Key::V && press.modifiers.ctrl {
                    pasting.ask_to_paste();
                    return true;
                }
                if (press.key == Key::Delete || press.key == Key::Backspace)
                    && let Some(id) = pasting.selected.get_untracked() {
                        pasting.remove_point(id);
                        return true;
                    }
                false
            }}
        >
            <ClickCatcher
                cursor=CursorIcon::Default
                on_press={move |press: PointerPress| pressing.press(press.pos)}
                on_drag={move |press: PointerPress| dragging.drag(press.pos)}
                on_active_change={move |active: bool| {
                    if !active {
                        releasing.release();
                    }
                }}
                on_pan_drag={move |delta: Vec2| panning.pan(delta)}
                children={children}
            />
        </Focusable>
    }
}
