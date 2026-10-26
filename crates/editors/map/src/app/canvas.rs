use std::rc::Rc;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    Color32, CursorIcon, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, TextAlign, Vec2,
};
use map_types::MapPoint;
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
const ATTRIBUTION_HEIGHT: Size = Size::Xs;

#[component]
pub(crate) fn MapCanvas(state: MapState) -> NodeId {
    let revision = state.revision;
    let sized = state.editor().world();
    let laid_out = state.editor().placed();
    let world = create_memo(move || {
        state.displayed_region.with(|_| ());
        sized.with(|_| ());
        laid_out.with(|_| ());
        state.anchor.with(|_| ());
        state.view()
    });
    let camera = create_memo(move || state.camera());
    let scale = create_memo(move || camera.get().map_or(1.0, |camera| camera.scale));
    let visible = create_memo(move || {
        sized.with(|_| ());
        state.visible()
    });
    let covering = create_memo(move || {
        let (world, visible, scale) = (world.get(), visible.get(), scale.get());
        match state.sized() {
            true => tiles::covering(world, visible, scale),
            false => Vec::new(),
        }
    });
    create_effect(move || {
        let shown = covering.get();
        untrack(|| state.use_tiles(&shown));
    });

    let measures = Rc::new(labels::Measures::default());
    let candidates = create_memo(move || {
        let _ = revision.get();
        let covering = covering.get();
        let world = world.get();
        state.with_tiles(|held| {
            let sources = labels::sources(held, &covering);
            Rc::new(labels::candidates(held, &sources, world, &measures))
        })
    });
    let decluttered =
        create_memo(move || Rc::new(labels::declutter(&candidates.get(), scale.get())));
    let shown_labels =
        create_memo(move || labels::shown(&decluttered.get(), visible.get(), scale.get()));
    let label_keys = create_memo(move || {
        shown_labels.with(|shown| shown.iter().map(labels::Label::key).collect::<Vec<_>>())
    });
    let by_key = create_memo(move || shown_labels.with(|shown| labels::by_key(shown)));

    let outline = create_memo(move || {
        let view = world.get();
        state
            .preview_region
            .get()
            .map(|preview| view.region_rect(preview))
    });
    let has_region = create_memo(move || outline.get().is_some());
    let region_rect = create_memo(move || outline.get().unwrap_or(Rect::ZERO));

    let points = state.points;
    let ids = create_memo(move || {
        points
            .get()
            .iter()
            .map(|point| point.id)
            .collect::<Vec<Uuid>>()
    });
    let selection = create_selector(move || state.selected.get());

    let content = NodeRef::new();
    state.editor().content(&content);

    view! {
        <List spacing=Space::None>
            <MapSurface state={state} @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <Canvas view={camera} @test_id={"map.canvas"}>
                    <ForEach keys={covering}>|tile: TileId| {
                        let world = world;
                        let scale = scale;
                        view! {
                            <Tile state tile world scale underlay=true />
                        }
                    }</ForEach>
                    <ForEach keys={covering}>|tile: TileId| {
                        let scale = scale;
                        view! {
                            <Tile state tile world scale />
                        }
                    }</ForEach>
                    <ForEach keys={label_keys}>|key: labels::LabelKey|
                        <MapLabel key shown={by_key} scale />
                    </ForEach>
                    <RegionOutline shown={has_region} rect={region_rect} />
                    <ForEach keys={ids}>|id: Uuid| {
                        let selected = selection.memo(Some(id));
                        let world = world;
                        let scale = scale;
                        view! {
                            <PointMarker state id selected world scale />
                        }
                    }</ForEach>
                </Canvas>
            </MapSurface>
            <Attribution />
        </List>
    }
}

#[component]
fn RegionOutline(shown: Memo<bool>, rect: Memo<Rect>) -> DynamicSegment<CanvasItem> {
    let x = create_memo(move || rect.get().left());
    let y = create_memo(move || rect.get().top());
    let width = create_memo(move || rect.get().width());
    let height = create_memo(move || rect.get().height());
    view! {
        <Show condition={shown}>
            <CanvasItem x={x} y={y} width={width} height={height}>
                <Frame border={Border::new(ColorRole::Custom(REGION_COLOR)).width(Stroke::Focus)} />
            </CanvasItem>
        </Show>
    }
}

#[component]
fn PointMarker(
    state: MapState,
    id: Uuid,
    selected: Memo<bool>,
    world: Memo<MapView>,
    scale: Memo<f32>,
) -> CanvasItem {
    let point = create_memo(move || state.points.get().into_iter().find(|point| point.id == id));
    let tip = create_memo(move || {
        let view = world.get();
        point
            .get()
            .map(|point: MapPoint| view.position(point.position))
            .unwrap_or(Pos2::ZERO)
    });
    let color = create_memo(move || {
        point.get().map_or(Color32::TRANSPARENT, |point| {
            points::marker_color(point.color)
        })
    });
    let label = create_memo(move || {
        point
            .get()
            .map_or_else(String::new, |point| state.name_of(point.block_id))
    });
    view! {
        <Marker tip={tip} scale={scale} color={color} selected={selected} label={label} />
    }
}

#[component]
fn Attribution() -> NodeId {
    view! {
        <Frame
            height=ATTRIBUTION_HEIGHT
            fill={ColorRole::Custom(HALO)}
            padding={Padding::horizontal(Space::Sm)}
        >
            <Text style=TextStyle::CAPTION align=TextAlign::End>{ATTRIBUTION}</Text>
        </Frame>
    }
}

#[component]
fn MapSurface(state: MapState, children: Option<Child>) -> NodeId {
    view! {
        <Interactive
            focusable=true
            pastes={vec![PasteKind::Image]}
            on_paste={move |pasted: Pasted| state.take_paste(pasted)}
            on_key={move |press: KeyPress| {
                if (press.key == Key::Delete || press.key == Key::Backspace)
                    && let Some(id) = state.selected.get_untracked() {
                        state.remove_point(id);
                        return true;
                    }
                false
            }}
            cursor=CursorIcon::Default
            on_press={move |press: PointerPress| state.press(press.pos)}
            on_drag={move |press: PointerPress| state.drag(press.pos)}
            on_active_change={move |active: bool| {
                if !active {
                    state.release();
                }
            }}
            on_pan_drag={move |delta: Vec2| state.pan(delta)}
            children={children}
        />
    }
}
