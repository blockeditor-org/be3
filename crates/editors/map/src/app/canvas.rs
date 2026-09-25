use std::rc::Rc;

use block_editor_plugin::be_block::map::MapPoint;
use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, Child, ClickCatcher, Focusable, ForEach, Frame, ItemSize, List, Memo, Text,
    clone, component, create_memo, create_selector, view,
};
use block_editor_plugin::beui::styled::{Caption, use_theme};
use block_editor_plugin::beui::{
    Color32, CursorIcon, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, TextAlign, Vec2,
};
use uuid::Uuid;

use crate::points::{self, Marker};
use crate::raster::TILE_PIXELS;

use super::state::MapState;
use super::tiles::{self, Placed, Tile};

pub(crate) const REGION_COLOR: Color32 = Color32::from_rgb(245, 180, 60);
const ATTRIBUTION: &str = "© OpenStreetMap contributors";
const HALO: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
const ATTRIBUTION_HEIGHT: f32 = 16.0;
const LABEL_WIDTH: f32 = 140.0;
const LABEL_HEIGHT: f32 = 16.0;
const LABEL_MARGIN: f32 = 40.0;

#[component]
pub(crate) fn MapCanvas(state: Rc<MapState>) -> NodeId {
    let placed = Rc::clone(&state);
    let revision = state.revision.clone();
    let world = create_memo(clone!(placed revision -> move || {
        let _ = revision.get();
        placed.world_rect()
    }));
    let clipped = Rc::clone(&state);
    let clip = create_memo(clone!(clipped revision -> move || {
        let _ = revision.get();
        clipped.content_rect()
    }));
    let covering =
        create_memo(clone!(world clip -> move || tiles::covering(world.get(), clip.get())));

    let labelled = Rc::clone(&state);
    let labels = create_memo(clone!(covering world clip revision labelled -> move || {
        let _ = revision.get();
        let (world, clip) = (world.get(), clip.get());
        let held = labelled.tiles();
        let mut placed = Vec::new();
        for tile in covering.get() {
            let Some(super::state::TileState::Ready { labels, .. }) = held.get(&tile.id) else {
                continue;
            };
            let rect = tiles::tile_rect(world, tile);
            for (index, label) in labels.iter().enumerate() {
                let at = Pos2::new(
                    rect.left() + label.position[0] * rect.width() / TILE_PIXELS as f32,
                    rect.top() + label.position[1] * rect.height() / TILE_PIXELS as f32,
                );
                if !clip.expand(LABEL_MARGIN).contains(at) {
                    continue;
                }
                placed.push(PlacedLabel {
                    tile: tile.id,
                    index,
                    at,
                    text: label.text.clone(),
                    size: label.font_size,
                    color: Color32::from_rgb(label.color[0], label.color[1], label.color[2]),
                });
            }
        }
        placed
    }));

    let region = Rc::clone(&state);
    let outline = create_memo(clone!(region revision world -> move || {
        let _ = revision.get();
        let _ = world.get();
        region
            .preview_region
            .get()
            .map(|preview| region.view().region_rect(preview))
    }));
    let has_region = create_memo(clone!(outline -> move || outline.get().is_some()));
    let region_rect = create_memo(clone!(outline -> move || outline.get().unwrap_or(Rect::ZERO)));

    let points = state.points.clone();
    let ids = create_memo(clone!(points -> move || {
        points.get().iter().map(|point| point.id).collect::<Vec<Uuid>>()
    }));
    let selection = create_selector(clone!(state -> move || state.selected.get()));

    let view = state.editor().canvas();
    let theme = use_theme();
    let markers = Rc::clone(&state);
    let interaction = Rc::clone(&state);

    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <MapSurface state={interaction} @sizing=ItemSize::Percent(100.0)>
                    <Canvas view={view} @test_id={"map.canvas"}>
                        <ForEach keys={covering}>
                            {move |tile: Placed| {
                                let state = Rc::clone(&placed);
                                let world = world.clone();
                                view! {
                                    <Tile state tile world />
                                }
                            }}
                        </ForEach>
                        <ForEach keys={labels}>
                            {move |label: PlacedLabel| {
                                view! {
                                    <MapLabel label />
                                }
                            }}
                        </ForEach>
                        <RegionOutline shown={has_region} rect={region_rect} />
                        <ForEach keys={ids}>
                            {move |id: Uuid| {
                                let state = Rc::clone(&markers);
                                let selected = selection.memo(Some(id));
                                view! {
                                    <PointMarker state id selected />
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

#[derive(Clone, PartialEq)]
pub(crate) struct PlacedLabel {
    tile: crate::tiles::TileId,
    index: usize,
    at: Pos2,
    text: String,
    size: f32,
    color: Color32,
}

impl std::hash::Hash for PlacedLabel {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.tile.hash(hasher);
        self.index.hash(hasher);
    }
}

impl Eq for PlacedLabel {}

#[component]
fn MapLabel(label: PlacedLabel) -> CanvasItem {
    view! {
        <CanvasItem
            x={label.at.x - LABEL_WIDTH / 2.0}
            y={label.at.y - LABEL_HEIGHT / 2.0}
            width=LABEL_WIDTH
            height=LABEL_HEIGHT
        >
            <TileName text={label.text} size={label.size} color={label.color} />
        </CanvasItem>
    }
}

#[component]
fn TileName(text: String, size: f32, color: Color32) -> NodeId {
    view! {
        <Frame color=HALO radius=2 padding_horizontal=2.0>
            <Text string={text} font_size={size} align=TextAlign::Center color={color} wrap=false />
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
fn PointMarker(state: Rc<MapState>, id: Uuid, selected: Memo<bool>) -> CanvasItem {
    let held = Rc::clone(&state);
    let point = create_memo(clone!(held -> move || {
        held.points.get().into_iter().find(|point| point.id == id)
    }));
    let placed = Rc::clone(&state);
    let revision = state.revision.clone();
    let tip = create_memo(clone!(point placed revision -> move || {
        let _ = revision.get();
        point
            .get()
            .map(|point: MapPoint| placed.view().position(point.position))
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
        <Marker tip={tip} color={color} selected={selected} label={label} />
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
