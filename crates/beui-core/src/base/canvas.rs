use std::any::Any;
use std::rc::Rc;
use std::time::{Duration, Instant};

use foldhash::{HashMap, HashSet};

use crate::callback::Callback;
use crate::child_list::{ChildHost, ChildList, NodeChildren};
use crate::current::with_document;
use crate::document::Document;
use crate::geometry::{Rect, Vec2, pos2};
use crate::node::{ANCESTOR_LIMIT, Element, NodeId, NodeOf, Rects, SpaceId};
use crate::painter::{Painter, Zoom};

pub use beui_types::layout::CanvasView;

impl ChildHost for CanvasNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }

    fn children_changed(&mut self) {
        self.index.stale = true;
        let items: std::collections::HashSet<NodeId> = self.items.nodes().into_iter().collect();
        self.laid.retain(|(item, _)| items.contains(item));
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
    stale: bool,
    read: u64,
    listed: Vec<NodeId>,
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
        self.relist(doc, items);
    }

    fn relist(&mut self, doc: &Document, items: &ChildList<NodeId>) {
        self.stale = false;
        let items: Vec<NodeId> = items.iter().copied().collect();
        let old = std::mem::take(&mut self.listed);
        let start = old.iter().zip(&items).take_while(|(a, b)| a == b).count();
        let end = old[start..]
            .iter()
            .rev()
            .zip(items[start..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let added = &items[start..items.len() - end];
        let kept: HashSet<NodeId> = added.iter().copied().collect();
        for item in &old[start..old.len() - end] {
            if !kept.contains(item) {
                self.order.remove(item);
                self.forget(*item);
            }
        }
        for (at, item) in items.iter().enumerate().skip(start) {
            self.order.insert(*item, at);
            if !self.held.contains_key(item) {
                self.insert(*item, doc.canvas_item_rect(NodeOf::assumed(*item)));
            }
        }
        self.listed = items;
    }

    fn forget(&mut self, item: NodeId) {
        match self.held.remove(&item) {
            Some(Some(held)) => held.each(|cell| {
                if let Some(items) = self.cells.get_mut(&cell) {
                    items.retain(|other| *other != item);
                    if items.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }),
            Some(None) => self.large.retain(|other| *other != item),
            None => {}
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
        if Cells::of(rect) == held {
            return;
        }
        self.forget(item);
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
    laid: Vec<(NodeId, u8)>,
    host: Option<NodeId>,
    shift: Vec2,
    zoom: Zoom,
    scrolls_across: bool,
    on_reveal: Callback<Vec2>,
    revealing: Option<Revealing>,
}

struct Revealing {
    target: NodeId,
    since: Instant,
    view: Option<CanvasView>,
    distance: f32,
}

const REVEAL_FOR: Duration = Duration::from_secs(1);

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

impl CanvasNode {
    fn follow_reveal(&mut self, doc: &mut Document, host: NodeId) {
        if let Some(target) = doc.take_reveal(host) {
            self.revealing = Some(Revealing {
                target,
                since: doc.now(),
                view: None,
                distance: f32::INFINITY,
            });
        }
        let Some(revealing) = self.revealing.as_mut() else {
            return;
        };
        if doc.now().saturating_duration_since(revealing.since) > REVEAL_FOR
            || self.on_reveal.is_empty()
        {
            self.revealing = None;
            return;
        }
        if self.view.is_none() || revealing.view == self.view {
            return;
        }
        revealing.view = self.view;
        let (target, distance, on_reveal) = (revealing.target, revealing.distance, self.on_reveal);
        doc.report_after_layout(move || {
            let delta = with_document(|doc| {
                let delta = doc
                    .canvas_reveal(host, target)
                    .filter(|delta| delta.length() < distance);
                let canvas = doc.arena.state_mut(host).as_any_mut();
                if let Some(canvas) = canvas.downcast_mut::<CanvasNode>() {
                    match delta {
                        Some(delta) => {
                            if let Some(revealing) = canvas.revealing.as_mut() {
                                revealing.distance = delta.length();
                            }
                        }
                        None => canvas.revealing = None,
                    }
                }
                delta
            });
            if let Some(delta) = delta {
                on_reveal.call(delta);
            }
        });
    }
}

impl Document {
    fn canvas_reveal(&self, canvas: NodeId, target: NodeId) -> Option<Vec2> {
        let rects = Rc::clone(&self.rects);
        let node = std::iter::successors(Some(target), |node| self.arena.parent(*node))
            .take(ANCESTOR_LIMIT)
            .take_while(|node| *node != canvas)
            .find(|node| rects.contains_key(node))?;
        let extent = self.outlined_extent(&rects, node)?;
        let shown = clear_of(
            rects.get(&canvas)?,
            &self.floating_rects(self.window_of(canvas)),
        );
        let caret = (Some(target) == self.focused_node())
            .then(|| self.ime_cursor_in(&rects, target))
            .flatten();
        let fits = extent.width() <= shown.width() && extent.height() <= shown.height();
        let wanted = match (fits, caret) {
            (false, Some(caret)) => caret,
            _ => extent,
        };
        let delta = Vec2::new(
            revealing(wanted.min.x, wanted.max.x, shown.min.x, shown.max.x),
            revealing(wanted.min.y, wanted.max.y, shown.min.y, shown.max.y),
        );
        (delta.length() >= 0.5).then_some(delta)
    }
}

fn clear_of(shown: Rect, covers: &[Rect]) -> Rect {
    covers
        .iter()
        .filter(|cover| cover.intersects(shown))
        .fold(shown, |free, cover| {
            let mut sides = [free; 4];
            sides[0].min.y = cover.max.y.min(free.max.y);
            sides[1].max.y = cover.min.y.max(free.min.y);
            sides[2].min.x = cover.max.x.min(free.max.x);
            sides[3].max.x = cover.min.x.max(free.min.x);
            sides
                .into_iter()
                .max_by(|a, b| area(*a).total_cmp(&area(*b)))
                .unwrap_or(free)
        })
}

fn area(rect: Rect) -> f32 {
    rect.width().max(0.0) * rect.height().max(0.0)
}

fn revealing(start: f32, end: f32, shown_start: f32, shown_end: f32) -> f32 {
    if end - start > shown_end - shown_start || start < shown_start {
        shown_start - start
    } else if end > shown_end {
        shown_end - end
    } else {
        0.0
    }
}

fn item_holding(doc: &Document, host: NodeId, node: NodeId) -> Option<NodeId> {
    let mut current = node;
    for _ in 0..ANCESTOR_LIMIT {
        let parent = doc.arena.parent(current)?;
        if parent == host {
            return Some(current);
        }
        current = parent;
    }
    None
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
        let painted = (self.host, self.shift, self.zoom);
        let was_laid = std::mem::take(&mut self.laid);
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
                if self.index.stale {
                    self.index.relist(doc, &self.items);
                }
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
        let mut near = self
            .index
            .near(seen)
            .unwrap_or_else(|| self.items.iter().copied().collect());
        let focused = doc
            .focused_node()
            .and_then(|focused| item_holding(doc, host, focused));
        if let Some(focused) = focused
            && !near.contains(&focused)
        {
            near.push(focused);
        }
        for item in &near {
            let world = doc.canvas_item_rect(NodeOf::assumed(*item));
            let slot = Self::slot(doc, NodeOf::assumed(*item));
            let (painter, placed, shown) = match slot {
                SCALED => (&scaled, world, seen),
                CLIPPED => (&clipped, scaled_rect(world, zoom.scale), visible),
                _ => (&unclipped, scaled_rect(world, zoom.scale), visible),
            };
            if placed.intersects(shown) || focused == Some(*item) {
                crate::layout::layout(doc, painter, *item, placed, out);
                self.laid.push((*item, slot));
            }
        }
        if was_laid != self.laid || painted != (self.host, self.shift, self.zoom) {
            doc.arena.note_relaid(host);
        }
        self.follow_reveal(doc, host);
    }

    fn paints_from_layout(&self) -> bool {
        false
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
        let zoom = Zoom {
            raster: self.zoom().raster,
            ..self.zoom
        };
        let scaled = painter.zoomed(SpaceId::inside(host, SCALED), self.shift, zoom, rect);
        for (item, slot) in &self.laid {
            if rects.contains_key(item) {
                let painter = match *slot {
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

    fn children_revision(&self) -> Option<u64> {
        Some(self.items.revision())
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        self.laid.iter().map(|(item, _)| *item).collect()
    }

    fn for_each_shown_child(&self, visit: &mut dyn FnMut(NodeId)) {
        for (item, _) in &self.laid {
            visit(*item);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::child_list::remove_hosted(self, child);
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
        self.child.filter(|_| self.pointer).into_iter().collect()
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
            on_reveal: Callback::empty(),
            revealing: None,
        })
    }

    pub fn set_canvas_on_reveal(&mut self, canvas: NodeOf<CanvasNode>, on_reveal: Callback<Vec2>) {
        self.arena.touch_mut_as::<CanvasNode>(canvas).on_reveal = on_reveal;
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
            self.arena.paint_mut_as(canvas).raster = raster;
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
        }
    }

    pub fn canvas_item_rect(&self, item: NodeOf<CanvasItemNode>) -> Rect {
        self.arena.get_as::<CanvasItemNode>(item).rect
    }
}
