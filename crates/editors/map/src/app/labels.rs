use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    CanvasItem, Frame, Memo, Text, clone, component, create_memo, layout_text, view,
};
use block_editor_beui::beui::{Color32, FontId, Pos2, Rect, TextAlign, TextLayout, Vec2};

use crate::geo::MapView;
use crate::raster::TILE_PIXELS;
use crate::tiles::TileId;

use super::state::TileState;
use super::tiles;

const HALO: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
const HALO_PADDING: f32 = 2.0;
const LABEL_GAP: f32 = 4.0;
const CELL: f32 = 64.0;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct LabelKey {
    text: String,
    occurrence: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Label {
    key: LabelKey,
    at: Pos2,
    size: Vec2,
    font_size: f32,
    color: Color32,
}

impl Label {
    pub(crate) fn key(&self) -> LabelKey {
        self.key.clone()
    }
}

pub(crate) fn sources(held: &HashMap<TileId, TileState>, covering: &[TileId]) -> Vec<TileId> {
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    for tile in covering {
        let mut source = Some(*tile);
        while let Some(id) = source {
            if matches!(held.get(&id), Some(TileState::Ready { .. })) {
                if seen.insert(id) {
                    sources.push(id);
                }
                break;
            }
            source = id.parent();
        }
    }
    sources
}

#[derive(Default)]
pub(crate) struct Measures(RefCell<HashMap<(String, u32), Vec2>>);

impl Measures {
    fn size(&self, text: &str, font_size: f32) -> Vec2 {
        let key = (text.to_owned(), font_size.to_bits());
        if let Some(size) = self.0.borrow().get(&key) {
            return *size;
        }
        let measured = layout_text(text, FontId::proportional(font_size), TextLayout::DEFAULT)
            .map(|galley| galley.size());
        let size = measured.unwrap_or_else(|| {
            Vec2::new(
                text.chars().count() as f32 * font_size * 0.55,
                font_size * 1.3,
            )
        });
        let size = size + Vec2::new(HALO_PADDING * 2.0, 0.0);
        if measured.is_some() {
            self.0.borrow_mut().insert(key, size);
        }
        size
    }
}

pub(crate) fn candidates(
    held: &HashMap<TileId, TileState>,
    sources: &[TileId],
    view: MapView,
    measures: &Measures,
) -> Vec<Label> {
    let mut found = Vec::new();
    for source in sources {
        let Some(TileState::Ready { labels, .. }) = held.get(source) else {
            continue;
        };
        for label in labels.iter() {
            let fraction = [
                label.position[0] / TILE_PIXELS as f32,
                label.position[1] / TILE_PIXELS as f32,
            ];
            found.push((label, tiles::tile_point(view, *source, fraction)));
        }
    }
    found.sort_by(|(a, a_at), (b, b_at)| {
        b.font_size
            .total_cmp(&a.font_size)
            .then_with(|| a.text.cmp(&b.text))
            .then_with(|| a_at.x.total_cmp(&b_at.x))
            .then_with(|| a_at.y.total_cmp(&b_at.y))
    });
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    found
        .into_iter()
        .map(|(label, at)| {
            let occurrence = occurrences.entry(label.text.as_str()).or_default();
            *occurrence += 1;
            Label {
                key: LabelKey {
                    text: label.text.clone(),
                    occurrence: *occurrence,
                },
                at,
                size: measures.size(&label.text, label.font_size),
                font_size: label.font_size,
                color: Color32::from_rgb(label.color[0], label.color[1], label.color[2]),
            }
        })
        .collect()
}

pub(crate) fn declutter(candidates: &[Label], scale: f32) -> Vec<Label> {
    let scale = scale.max(f32::EPSILON);
    let mut grid: HashMap<(i32, i32), Vec<Rect>> = HashMap::new();
    let mut kept = Vec::new();
    for label in candidates {
        let centre = Pos2::new(label.at.x * scale, label.at.y * scale);
        let rect = Rect::from_center_size(centre, label.size + Vec2::splat(LABEL_GAP));
        let cells = || {
            let columns =
                (rect.left() / CELL).floor() as i32..=(rect.right() / CELL).floor() as i32;
            let rows = (rect.top() / CELL).floor() as i32..=(rect.bottom() / CELL).floor() as i32;
            rows.flat_map(move |row| columns.clone().map(move |column| (column, row)))
        };
        let blocked = cells().any(|cell| {
            grid.get(&cell)
                .is_some_and(|rects| rects.iter().any(|other| other.intersects(rect)))
        });
        if blocked {
            continue;
        }
        for cell in cells() {
            grid.entry(cell).or_default().push(rect);
        }
        kept.push(label.clone());
    }
    kept
}

pub(crate) fn shown(placed: &[Label], visible: Rect, scale: f32) -> Vec<Label> {
    let scale = scale.max(f32::EPSILON);
    placed
        .iter()
        .filter(|label| {
            let half = label.size / (2.0 * scale);
            visible.intersects(Rect::from_min_max(label.at - half, label.at + half))
        })
        .cloned()
        .collect()
}

pub(crate) fn by_key(shown: &[Label]) -> Rc<HashMap<LabelKey, Label>> {
    Rc::new(
        shown
            .iter()
            .map(|label| (label.key.clone(), label.clone()))
            .collect(),
    )
}

#[component]
pub(crate) fn MapLabel(
    key: LabelKey,
    shown: Memo<Rc<HashMap<LabelKey, Label>>>,
    scale: Memo<f32>,
) -> CanvasItem {
    let test_id = format!("map.label.{}.{}", key.text, key.occurrence);
    let label = create_memo(clone!(shown -> move || shown.with(|shown| shown.get(&key).cloned())));
    let rect = create_memo(clone!(label scale -> move || {
        let scale = scale.get().max(f32::EPSILON);
        label.get().map_or(Rect::ZERO, |label| {
            Rect::from_center_size(label.at, label.size / scale)
        })
    }));
    let text = create_memo(clone!(label -> move || {
        label.get().map(|label| label.key.text).unwrap_or_default()
    }));
    let font_size =
        create_memo(clone!(label -> move || label.get().map_or(10.0, |label| label.font_size)));
    let color = create_memo(
        clone!(label -> move || label.get().map_or(Color32::TRANSPARENT, |label| label.color)),
    );
    let x = create_memo(clone!(rect -> move || rect.get().left()));
    let y = create_memo(clone!(rect -> move || rect.get().top()));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame color=HALO radius=2 padding_horizontal=HALO_PADDING @test_id={test_id}>
                <Text
                    string={text}
                    font_size={font_size}
                    align=TextAlign::Center
                    color={color}
                    wrap=false
                />
            </Frame>
        </CanvasItem>
    }
}
