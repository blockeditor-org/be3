use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    CanvasItem, Frame, Picture, clone, component, create_memo, view,
};
use block_editor_plugin::beui::{Color32, ImageFit, Pos2, Rect, Vec2};

use crate::tiles::{SOURCE_MAX_ZOOM, TileId};

use super::state::{MapState, TileState};

pub(crate) const TILE_POINTS: f32 = 256.0;
pub(crate) const BACKGROUND: Color32 = Color32::from_rgb(242, 239, 233);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Placed {
    pub(crate) id: TileId,
    pub(crate) column: u32,
    pub(crate) row: u32,
}

pub(crate) fn covering(world: Rect, clip: Rect) -> Vec<Placed> {
    let visible = clip.intersect(world);
    if !visible.is_positive() {
        return Vec::new();
    }
    let zoom = (world.width().max(1.0) / TILE_POINTS).log2();
    let tile_zoom = zoom.floor().clamp(0.0, f32::from(SOURCE_MAX_ZOOM)) as u8;
    let count = 1u32 << tile_zoom;
    let side = world.width() / count as f32;
    let range = |from: f32, to: f32| {
        let from = ((from / side).floor().max(0.0) as u32).min(count);
        let to = ((to / side).ceil().max(0.0) as u32).min(count);
        from..to
    };
    let columns = range(
        visible.left() - world.left(),
        visible.right() - world.left(),
    );
    let rows = range(visible.top() - world.top(), visible.bottom() - world.top());
    let mut placed = Vec::new();
    for row in rows {
        for column in columns.clone() {
            placed.push(Placed {
                id: TileId {
                    zoom: tile_zoom,
                    x: column,
                    y: row,
                },
                column,
                row,
            });
        }
    }
    placed
}

pub(crate) fn tile_rect(world: Rect, placed: Placed) -> Rect {
    let count = 1u32 << placed.id.zoom;
    let side = world.width() / count as f32;
    Rect::from_min_size(
        Pos2::new(
            world.left() + placed.column as f32 * side,
            world.top() + placed.row as f32 * side,
        ),
        Vec2::splat(side),
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

#[component]
pub(crate) fn Tile(
    state: Rc<MapState>,
    tile: Placed,
    world: block_editor_plugin::beui::reactive::Memo<Rect>,
) -> CanvasItem {
    let wanted = Rc::clone(&state);
    wanted.want_tile(tile.id);
    let revision = state.revision.clone();
    let drawn = create_memo(clone!(revision -> move || {
        let _ = revision.get();
        let tiles = state.tiles();
        if let Some(image) = tiles.get(&tile.id).and_then(TileState::image) {
            return Some((image.clone(), None));
        }
        let mut ancestor = tile.id;
        while let Some(parent) = ancestor.parent() {
            ancestor = parent;
            if let Some(image) = tiles.get(&ancestor).and_then(TileState::image) {
                return Some((image.clone(), Some(magnified(tile.id, ancestor))));
            }
        }
        None
    }));
    let image = create_memo(clone!(drawn -> move || drawn.get().map(|(image, _)| image)));
    let source = create_memo(clone!(drawn -> move || drawn.get().and_then(|(_, uv)| uv)));
    let rect = create_memo(clone!(world -> move || tile_rect(world.get(), tile)));
    let x = create_memo(clone!(rect -> move || rect.get().left()));
    let y = create_memo(clone!(rect -> move || rect.get().top()));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame color=BACKGROUND>
                <Picture image={image} source={source} fit=ImageFit::Fill />
            </Frame>
        </CanvasItem>
    }
}
