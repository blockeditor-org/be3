use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_GRID_3X3;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, Painter, Pos2, Rect, Vec2, pos2};
use infinite_canvas_types::{CanvasGrid, CanvasGridSpacing};

use super::paint::with_opacity;
use super::state::CanvasState;

const DOT_RADIUS: f32 = 1.0;
const FADED_GAP: f32 = 8.0;
const FULL_GAP: f32 = 12.0;
const TILE_DOTS: i32 = 32;
const ZOOM_STEPS: f32 = 4.0;
const MOST_TILES: i64 = 400;

#[derive(Clone)]
pub(crate) struct GridActions {
    pub(crate) shown: Action,
    pub(crate) small: Action,
    pub(crate) medium: Action,
    pub(crate) large: Action,
}

pub(crate) fn grid_actions(state: CanvasState) -> GridActions {
    let grid = state.grid();
    let live = !state.previewing();
    let toggle = Action::new("canvas.grid", "Show grid", move || {
        let grid = grid.get_untracked();
        state.set_grid(CanvasGrid {
            hidden: !grid.hidden,
            ..grid
        });
    })
    .glyph(ICON_GRID_3X3)
    .checked(create_memo(move || !grid.get().hidden))
    .enabled(live)
    .register();
    let spaced = |id: &str, label: &'static str, spacing: CanvasGridSpacing| {
        Action::new(id, label, move || {
            state.set_grid(CanvasGrid {
                hidden: false,
                spacing,
            })
        })
        .radio(create_memo(move || grid.get().shown() == Some(spacing)))
        .enabled(live)
        .register()
    };
    GridActions {
        shown: toggle,
        small: spaced("canvas.grid-small", "Small grid", CanvasGridSpacing::Small),
        medium: spaced(
            "canvas.grid-medium",
            "Medium grid",
            CanvasGridSpacing::Medium,
        ),
        large: spaced("canvas.grid-large", "Large grid", CanvasGridSpacing::Large),
    }
}

fn world_spacing(spacing: CanvasGridSpacing) -> f32 {
    match spacing {
        CanvasGridSpacing::Small => 12.0,
        CanvasGridSpacing::Medium => 24.0,
        CanvasGridSpacing::Large => 48.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridLook {
    pub(crate) step: f32,
    pub(crate) radius: f32,
    pub(crate) between: f32,
    pub(crate) color: Color32,
    origin: Pos2,
}

impl GridLook {
    pub(crate) fn new(spacing: CanvasGridSpacing, scale: f32, color: Color32) -> Option<Self> {
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        let zoom = ((scale.log2() * ZOOM_STEPS).round() / ZOOM_STEPS).exp2();
        let mut step = world_spacing(spacing);
        while step * zoom < FADED_GAP {
            step *= 2.0;
        }
        let between = ((step * zoom - FADED_GAP) / (FULL_GAP - FADED_GAP)).clamp(0.0, 1.0);
        Some(Self {
            step,
            radius: DOT_RADIUS / zoom,
            between,
            color,
            origin: Pos2::ZERO,
        })
    }

    fn tile(self) -> f32 {
        self.step * TILE_DOTS as f32
    }

    pub(crate) fn tiles(self, visible: Rect) -> Vec<GridTile> {
        let size = self.tile();
        let span = |from: f32, to: f32| {
            let first = (from / size).floor();
            let last = (to / size).floor();
            (first.is_finite() && last.is_finite()).then_some((first as i64, last as i64))
        };
        let (Some((left, right)), Some((top, bottom))) = (
            span(visible.min.x, visible.max.x),
            span(visible.min.y, visible.max.y),
        ) else {
            return Vec::new();
        };
        if (right - left + 1) * (bottom - top + 1) > MOST_TILES {
            return Vec::new();
        }
        let step = self.step.to_bits();
        (top..=bottom)
            .flat_map(|y| (left..=right).map(move |x| GridTile { step, x, y }))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct GridTile {
    step: u32,
    x: i64,
    y: i64,
}

impl GridTile {
    fn rect(self) -> Rect {
        let size = f32::from_bits(self.step) * TILE_DOTS as f32;
        Rect::from_min_size(
            pos2(self.x as f32 * size, self.y as f32 * size),
            Vec2::splat(size),
        )
    }
}

fn draw_tile(painter: &Painter, corner: Pos2, look: GridLook, inside: &Option<Vec<Rect>>) {
    let between = with_opacity(look.color, look.between);
    let size = Vec2::splat(look.radius * 2.0);
    for row in 0..TILE_DOTS {
        for column in 0..TILE_DOTS {
            let offset = Vec2::new(column as f32, row as f32) * look.step;
            if let Some(inside) = inside {
                let world = look.origin + offset;
                if !inside.iter().any(|frame| frame.contains(world)) {
                    continue;
                }
            }
            let color = match row % 2 == 0 && column % 2 == 0 {
                true => look.color,
                false => between,
            };
            let center = corner + offset;
            painter.rect_filled(Rect::from_center_size(center, size), look.radius, color);
        }
    }
}

#[component]
pub(crate) fn GridTileItem(
    tile: GridTile,
    look: Memo<Option<GridLook>>,
    frames: Memo<Vec<Rect>>,
) -> CanvasItem {
    let rect = tile.rect();
    let step = f32::from_bits(tile.step);
    let inside = create_memo(move || {
        frames.with(|frames| {
            (!frames.is_empty()).then(|| {
                frames
                    .iter()
                    .filter(|frame| frame.intersects(rect))
                    .copied()
                    .collect::<Vec<_>>()
            })
        })
    });
    let draw = create_signal::<Draw>(Rc::new(|_, _| {}));
    create_effect(move || {
        let Some(look) = look.get().filter(|look| look.step == step) else {
            return;
        };
        let look = GridLook {
            origin: rect.min,
            ..look
        };
        let inside = inside.get();
        draw.set_unconditionally(Rc::new(move |painter: &Painter, rect: Rect| {
            draw_tile(painter, rect.min, look, &inside)
        }));
    });
    view! {
        <CanvasItem
            x={rect.left()}
            y={rect.top()}
            width={rect.width()}
            height={rect.height()}
            clip=false
            pointer=false
            scales=true
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}
