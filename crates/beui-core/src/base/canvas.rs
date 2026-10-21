use std::any::Any;

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
}

pub struct CanvasNode {
    view: Option<CanvasView>,
    raster: Option<f32>,
    size: Vec2,
    items: ChildList<NodeId>,
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
        for item in self.items.iter() {
            let world = doc.canvas_item_rect(NodeOf::assumed(*item));
            let (painter, placed, shown) = match Self::slot(doc, NodeOf::assumed(*item)) {
                SCALED => (&scaled, world, seen),
                CLIPPED => (&clipped, scaled_rect(world, zoom.scale), visible),
                _ => (&unclipped, scaled_rect(world, zoom.scale), visible),
            };
            if placed.intersects(shown) {
                crate::layout::layout(doc, painter, *item, placed, out);
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
        for item in self.items.iter() {
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
