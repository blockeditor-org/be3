use std::any::Any;

use foldhash::{HashMap, HashSet};

use crate::base::child_list::{ChildHost, ChildList, NodeChildren};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::node::{Element, NodeId, NodeOf, Rects, SpaceId};
use crate::painter::{Painter, Zoom};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CanvasView {
    pub origin: Pos2,
    pub scale: f32,
}

impl CanvasView {
    pub fn new(origin: Pos2, scale: f32) -> Self {
        Self { origin, scale }
    }

    pub fn to_screen(self, point: Pos2) -> Pos2 {
        pos2(
            self.origin.x + point.x * self.scale,
            self.origin.y + point.y * self.scale,
        )
    }

    pub fn to_canvas(self, point: Pos2) -> Pos2 {
        pos2(
            (point.x - self.origin.x) / self.scale,
            (point.y - self.origin.y) / self.scale,
        )
    }

    pub fn rect_to_screen(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(rect.min), self.to_screen(rect.max))
    }

    pub fn rect_to_canvas(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_canvas(rect.min), self.to_canvas(rect.max))
    }
}

impl ChildHost for CanvasNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }

    fn children_changed(&mut self) {
        self.index.built = false;
    }
}

const CELL: f32 = 256.0;
const WIDEST: f32 = 16.0 * CELL;

#[derive(Clone, Copy, PartialEq)]
struct Cells {
    min: (i32, i32),
    max: (i32, i32),
}

impl Cells {
    fn of(rect: Rect) -> Option<Self> {
        let finite = [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
            .iter()
            .all(|value| value.is_finite());
        if !finite || rect.width() > WIDEST || rect.height() > WIDEST {
            return None;
        }
        let cell = |value: f32| (value / CELL).floor() as i32;
        Some(Self {
            min: (cell(rect.min.x), cell(rect.min.y)),
            max: (cell(rect.max.x), cell(rect.max.y)),
        })
    }

    fn count(self) -> usize {
        let across = (self.max.0 as i64 - self.min.0 as i64 + 1).max(0);
        let down = (self.max.1 as i64 - self.min.1 as i64 + 1).max(0);
        (across * down) as usize
    }

    fn each(self, mut visit: impl FnMut((i32, i32))) {
        for y in self.min.1..=self.max.1 {
            for x in self.min.0..=self.max.0 {
                visit((x, y));
            }
        }
    }
}

#[derive(Default)]
struct ItemIndex {
    built: bool,
    read: u64,
    order: HashMap<NodeId, usize>,
    held: HashMap<NodeId, Option<Cells>>,
    cells: HashMap<(i32, i32), Vec<NodeId>>,
    large: Vec<NodeId>,
}

impl ItemIndex {
    fn rebuild(&mut self, doc: &Document, items: &ChildList<NodeId>) {
        *self = Self {
            built: true,
            ..Self::default()
        };
        for (at, item) in items.iter().enumerate() {
            self.order.insert(*item, at);
            self.insert(*item, doc.canvas_item_rect(NodeOf::assumed(*item)));
        }
    }

    fn insert(&mut self, item: NodeId, rect: Rect) {
        let cells = Cells::of(rect);
        match cells {
            Some(cells) => cells.each(|cell| self.cells.entry(cell).or_default().push(item)),
            None => self.large.push(item),
        }
        self.held.insert(item, cells);
    }

    fn moved(&mut self, item: NodeId, rect: Rect) {
        let Some(held) = self.held.get(&item).copied() else {
            return;
        };
        let cells = Cells::of(rect);
        if cells == held {
            return;
        }
        match held {
            Some(held) => held.each(|cell| {
                if let Some(items) = self.cells.get_mut(&cell) {
                    items.retain(|other| *other != item);
                    if items.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }),
            None => self.large.retain(|other| *other != item),
        }
        self.insert(item, rect);
    }

    fn near(&self, seen: Rect) -> Option<Vec<NodeId>> {
        let cells = Cells::of(seen).filter(|cells| cells.count() <= self.order.len())?;
        let mut found: HashSet<NodeId> = self.large.iter().copied().collect();
        cells.each(|cell| {
            if let Some(items) = self.cells.get(&cell) {
                found.extend(items.iter().copied());
            }
        });
        let mut found: Vec<(usize, NodeId)> = found
            .into_iter()
            .filter_map(|item| Some((*self.order.get(&item)?, item)))
            .collect();
        found.sort_unstable_by_key(|(at, _)| *at);
        Some(found.into_iter().map(|(_, item)| item).collect())
    }
}

pub struct CanvasNode {
    view: Option<CanvasView>,
    raster: Option<f32>,
    size: Vec2,
    items: ChildList<NodeId>,
    index: ItemIndex,
    laid: Vec<NodeId>,
    host: Option<NodeId>,
    shift: Vec2,
    zoom: Zoom,
    scrolls_across: bool,
}

const CLIPPED: u8 = 1;
const UNCLIPPED: u8 = 2;
const SCALED: u8 = 3;

impl CanvasNode {
    pub fn pans(&self) -> bool {
        self.view.is_some()
    }

    pub fn scrolls_across(&self) -> bool {
        self.scrolls_across
    }

    fn shift(&self, doc: &Document, painter: &Painter) -> Vec2 {
        match self.view {
            Some(view) => doc
                .pixel_grid()
                .snap_vec(view.origin.to_vec2() - painter.origin()),
            None => Vec2::ZERO,
        }
    }

    fn scale(&self) -> f32 {
        self.view
            .map(|view| view.scale)
            .filter(|scale| *scale > 0.0 && scale.is_finite())
            .unwrap_or(1.0)
    }

    fn zoom(&self) -> Zoom {
        let scale = self.scale();
        Zoom {
            scale,
            raster: self
                .raster
                .filter(|raster| *raster > 0.0 && raster.is_finite())
                .unwrap_or(scale),
        }
    }

    fn slot(doc: &Document, item: NodeOf<CanvasItemNode>) -> u8 {
        let item = doc.arena.get_as::<CanvasItemNode>(item);
        match (item.scales, item.clip) {
            (true, _) => SCALED,
            (false, true) => CLIPPED,
            (false, false) => UNCLIPPED,
        }
    }
}

fn scaled_rect(rect: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * scale, rect.min.y * scale),
        pos2(rect.max.x * scale, rect.max.y * scale),
    )
}

impl Element for CanvasNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        self.size
    }

    fn relayout_boundary(&self) -> bool {
        true
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let host = doc.laying_out().expect("a canvas is laid out as itself");
        let shift = self.shift(doc, painter);
        let zoom = self.zoom();
        self.host = Some(host);
        self.shift = shift;
        self.zoom = zoom;
        let clipped = doc.enter_space(host, CLIPPED, painter, shift, rect, out);
        let unclipped = doc.enter_space(host, UNCLIPPED, painter, shift, Rect::EVERYTHING, out);
        let scaled = doc.enter_zoomed_space(host, SCALED, painter, shift, zoom, rect, out);
        let visible = rect.translate(-shift);
        let seen = visible.scaled(zoom.scale.recip());
        let unread = self
            .index
            .read
            .checked_sub(doc.canvas_moves_from)
            .and_then(|from| doc.canvas_moves.get(from as usize..));
        match unread {
            Some(moved) if self.index.built => {
                for item in moved {
                    if self.index.order.contains_key(item) && doc.arena.contains(*item) {
                        let rect = doc.canvas_item_rect(NodeOf::assumed(*item));
                        self.index.moved(*item, rect);
                    }
                }
            }
            _ => self.index.rebuild(doc, &self.items),
        }
        self.index.read = doc.canvas_moves_from + doc.canvas_moves.len() as u64;
        let near = self
            .index
            .near(seen)
            .unwrap_or_else(|| self.items.iter().copied().collect());
        self.laid.clear();
        for item in &near {
            let world = doc.canvas_item_rect(NodeOf::assumed(*item));
            let (painter, placed, shown) = match Self::slot(doc, NodeOf::assumed(*item)) {
                SCALED => (&scaled, world, seen),
                CLIPPED => (&clipped, scaled_rect(world, zoom.scale), visible),
                _ => (&unclipped, scaled_rect(world, zoom.scale), visible),
            };
            if placed.intersects(shown) {
                crate::layout::layout(doc, painter, *item, placed, out);
                self.laid.push(*item);
            }
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(host) = self.host else {
            return;
        };
        let clipped = painter.shifted(SpaceId::inside(host, CLIPPED), self.shift, rect);
        let unclipped = painter.shifted(
            SpaceId::inside(host, UNCLIPPED),
            self.shift,
            Rect::EVERYTHING,
        );
        let scaled = painter.zoomed(SpaceId::inside(host, SCALED), self.shift, self.zoom, rect);
        for item in &self.laid {
            if rects.contains_key(item) {
                let painter = match Self::slot(doc, NodeOf::assumed(*item)) {
                    SCALED => &scaled,
                    CLIPPED => &clipped,
                    _ => &unclipped,
                };
                crate::paint::paint(doc, painter, rects, *item);
            }
        }
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        self.laid.clone()
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::base::child_list::remove_hosted(self, child);
    }

    fn kind(&self) -> &'static str {
        "canvas"
    }

    fn detail(&self) -> Option<String> {
        let view = self.view?;
        Some(format!(
            "{:.0},{:.0} at {:.2}, text at {:.2}",
            view.origin.x,
            view.origin.y,
            view.scale,
            self.zoom().raster
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct CanvasItemNode {
    child: Option<NodeId>,
    rect: Rect,
    clip: bool,
    pointer: bool,
    scales: bool,
}

impl Element for CanvasItemNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        self.rect.size()
    }

    fn relayout_boundary(&self) -> bool {
        true
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        self.child
            .filter(|_| self.pointer && !self.scales)
            .into_iter()
            .collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "canvas item"
    }

    fn detail(&self) -> Option<String> {
        Some(format!(
            "{:.0},{:.0} {:.0}x{:.0}{}",
            self.rect.min.x,
            self.rect.min.y,
            self.rect.width(),
            self.rect.height(),
            match self.scales {
                true => " scaled",
                false => "",
            }
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn panned_by_canvas(&self, id: NodeId) -> bool {
        let mut at = self.arena.parent(id);
        while let Some(node) = at {
            if let Some(canvas) = self.arena.kind_of::<CanvasNode>(node) {
                return self.arena.get_as(canvas).pans();
            }
            at = self.arena.parent(node);
        }
        false
    }

    pub fn create_canvas(&mut self) -> NodeOf<CanvasNode> {
        self.arena.insert(CanvasNode {
            view: None,
            raster: None,
            size: Vec2::ZERO,
            items: ChildList::default(),
            index: ItemIndex::default(),
            laid: Vec::new(),
            host: None,
            shift: Vec2::ZERO,
            zoom: Zoom::NONE,
            scrolls_across: false,
        })
    }

    pub fn set_canvas_scrolls_across(&mut self, canvas: NodeOf<CanvasNode>, scrolls: bool) {
        if self.arena.get_as::<CanvasNode>(canvas).scrolls_across != scrolls {
            self.arena.get_mut_as::<CanvasNode>(canvas).scrolls_across = scrolls;
        }
    }

    pub fn set_canvas_view(&mut self, canvas: NodeOf<CanvasNode>, view: Option<CanvasView>) {
        if self.arena.get_as::<CanvasNode>(canvas).view != view {
            self.arena.get_mut_as::<CanvasNode>(canvas).view = view;
        }
    }

    pub fn set_canvas_raster(&mut self, canvas: NodeOf<CanvasNode>, raster: Option<f32>) {
        if self.arena.get_as::<CanvasNode>(canvas).raster != raster {
            self.arena.get_mut_as::<CanvasNode>(canvas).raster = raster;
        }
    }

    pub fn canvas_raster(&self, canvas: NodeOf<CanvasNode>) -> Option<f32> {
        self.arena.get_as::<CanvasNode>(canvas).raster
    }

    pub fn set_canvas_size(&mut self, canvas: NodeOf<CanvasNode>, size: Vec2) {
        if self.arena.get_as::<CanvasNode>(canvas).size != size {
            self.arena.get_mut_as::<CanvasNode>(canvas).size = size;
        }
    }

    pub fn create_canvas_item(&mut self) -> NodeOf<CanvasItemNode> {
        self.arena.insert(CanvasItemNode {
            child: None,
            rect: Rect::ZERO,
            clip: true,
            pointer: true,
            scales: false,
        })
    }

    pub fn set_canvas_item_child(&mut self, item: NodeOf<CanvasItemNode>, child: NodeId) {
        if self.arena.get_as::<CanvasItemNode>(item).child != Some(child) {
            self.arena.get_mut_as::<CanvasItemNode>(item).child = Some(child);
        }
    }

    pub fn set_canvas_item_rect(&mut self, item: NodeOf<CanvasItemNode>, rect: Rect) {
        if self.arena.get_as::<CanvasItemNode>(item).rect != rect {
            self.arena.get_mut_as::<CanvasItemNode>(item).rect = rect;
            self.canvas_moves.push(item.id());
        }
    }

    pub fn set_canvas_item_clip(&mut self, item: NodeOf<CanvasItemNode>, clip: bool) {
        if self.arena.get_as::<CanvasItemNode>(item).clip != clip {
            self.arena.get_mut_as::<CanvasItemNode>(item).clip = clip;
        }
    }

    pub fn set_canvas_item_pointer(&mut self, item: NodeOf<CanvasItemNode>, pointer: bool) {
        if self.arena.get_as::<CanvasItemNode>(item).pointer != pointer {
            self.arena.touch_mut_as::<CanvasItemNode>(item).pointer = pointer;
        }
    }

    pub fn set_canvas_item_scales(&mut self, item: NodeOf<CanvasItemNode>, scales: bool) {
        if self.arena.get_as::<CanvasItemNode>(item).scales != scales {
            self.arena.get_mut_as::<CanvasItemNode>(item).scales = scales;
            if let Some(canvas) = self.arena.parent(item.id()) {
                self.arena.invalidate_node(canvas);
            }
        }
    }

    pub fn canvas_item_rect(&self, item: NodeOf<CanvasItemNode>) -> Rect {
        self.arena.get_as::<CanvasItemNode>(item).rect
    }
}
