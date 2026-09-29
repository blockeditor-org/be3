use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    CanvasItem, Frame, Memo, Picture, clone, component, create_effect, create_memo, on_cleanup,
    untrack, view,
};
use block_editor_beui::beui::{Color32, Image, ImageFit, Pos2, Rect, Vec2};

use crate::geo::MapView;
use crate::tiles::{MAX_TILE_ZOOM, TileId};

use super::state::{MapState, TileState};

pub(crate) const TILE_POINTS: f32 = 256.0;
const MIN_TILE_ZOOM: u8 = 2;
const SEAM_BLEED: f32 = 1.0;
pub(crate) const BACKGROUND: Color32 = Color32::from_rgb(242, 239, 233);

pub(crate) fn tile_zoom(view: MapView, scale: f32) -> u8 {
    let shown = view.size().max(1.0) * f64::from(scale.max(f32::EPSILON));
    (shown / f64::from(TILE_POINTS))
        .log2()
        .round()
        .clamp(f64::from(MIN_TILE_ZOOM), f64::from(MAX_TILE_ZOOM)) as u8
}

pub(crate) fn covering(view: MapView, visible: Rect, scale: f32) -> Vec<TileId> {
    let zoom = tile_zoom(view, scale);
    let count = 1u32 << zoom;
    let range = |from: f64, to: f64| {
        let from = ((from * f64::from(count)).floor().max(0.0) as u32).min(count);
        let to = ((to * f64::from(count)).ceil().max(0.0) as u32).min(count);
        from..to
    };
    let min = view.normalized(visible.min);
    let max = view.normalized(visible.max);
    let columns = range(min[0], max[0]);
    let mut covering = Vec::new();
    for y in range(min[1], max[1]) {
        for x in columns.clone() {
            covering.push(TileId { zoom, x, y });
        }
    }
    covering
}

pub(crate) fn tile_point(view: MapView, tile: TileId, fraction: [f32; 2]) -> Pos2 {
    let count = f64::from(1u32 << tile.zoom);
    view.normalized_position([
        (f64::from(tile.x) + f64::from(fraction[0])) / count,
        (f64::from(tile.y) + f64::from(fraction[1])) / count,
    ])
}

pub(crate) fn tile_rect(view: MapView, tile: TileId) -> Rect {
    Rect::from_min_max(
        tile_point(view, tile, [0.0, 0.0]),
        tile_point(view, tile, [1.0, 1.0]),
    )
}

fn magnified(id: TileId, ancestor: TileId) -> Rect {
    let factor = 1u32 << (id.zoom - ancestor.zoom);
    let side = 1.0 / factor as f32;
    Rect::from_min_size(
        Pos2::new(
            (id.x - ancestor.x * factor) as f32 * side,
            (id.y - ancestor.y * factor) as f32 * side,
        ),
        Vec2::splat(side),
    )
}

fn drawn(state: Rc<MapState>, tile: TileId) -> Memo<Option<(Image, Option<Rect>)>> {
    let revision = state.revision.clone();
    create_memo(move || {
        let _ = revision.get();
        let tiles = state.tiles();
        if let Some(image) = tiles.get(&tile).and_then(TileState::image) {
            return Some((image.clone(), None));
        }
        let mut ancestor = tile;
        while let Some(parent) = ancestor.parent() {
            ancestor = parent;
            if let Some(image) = tiles.get(&ancestor).and_then(TileState::image) {
                return Some((image.clone(), Some(magnified(tile, ancestor))));
            }
        }
        None
    })
}

fn bled(rect: Rect, scale: f32) -> Rect {
    let precision = rect.max.x.abs().max(rect.max.y.abs()) * f32::EPSILON * 4.0;
    let bleed = (SEAM_BLEED / scale.max(f32::EPSILON)).max(precision);
    rect.expand(bleed)
}

#[component]
pub(crate) fn Tile(
    state: Rc<MapState>,
    tile: TileId,
    world: Memo<MapView>,
    scale: Memo<f32>,
    #[prop(default = false)] underlay: bool,
) -> CanvasItem {
    if !underlay {
        let wanted = Rc::clone(&state);
        create_effect(move || {
            wanted.reloads.with(|_| ());
            untrack(|| wanted.want_tile(tile));
        });
        let unwanted = Rc::clone(&state);
        on_cleanup(move || unwanted.unwant_tile(tile));
    }
    let drawn = drawn(state, tile);
    let image = create_memo(clone!(drawn -> move || drawn.get().map(|(image, _)| image)));
    let source = create_memo(clone!(drawn -> move || drawn.get().and_then(|(_, uv)| uv)));
    let backdrop = create_memo(
        clone!(drawn -> move || match underlay || drawn.with(Option::is_some) {
            true => Color32::TRANSPARENT,
            false => BACKGROUND,
        }),
    );
    let rect = create_memo(clone!(world scale -> move || {
        let rect = tile_rect(world.get(), tile);
        match underlay {
            true => bled(rect, scale.get()),
            false => rect,
        }
    }));
    let x = create_memo(clone!(rect -> move || rect.get().left()));
    let y = create_memo(clone!(rect -> move || rect.get().top()));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    let test_id = match underlay {
        true => format!("map.underlay.{}.{}.{}", tile.zoom, tile.x, tile.y),
        false => format!("map.tile.{}.{}.{}", tile.zoom, tile.x, tile.y),
    };
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame color={backdrop}>
                <Picture image={image} source={source} fit=ImageFit::Fill @test_id={test_id} />
            </Frame>
        </CanvasItem>
    }
}
