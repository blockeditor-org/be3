use std::any::Any;

use beui_macros::component;

use crate::base::child_list::{ChildHost, ChildItem, ChildList};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::node::{Element, InteractInput, NodeId, Rects, SpaceId};
use crate::painter::Painter;
use crate::reactive::{
    Child, ChildValue, Children, NodeSlot, Prop, Scope, SlotChild, create_effect, with_document,
};

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

pub(crate) struct CanvasNode {
    view: Option<CanvasView>,
    size: Vec2,
    items: ChildList<NodeId>,
    host: Option<NodeId>,
    shift: Vec2,
}

const CLIPPED: u8 = 1;
const UNCLIPPED: u8 = 2;

impl CanvasNode {
    fn shift(&self, doc: &Document, painter: &Painter) -> Vec2 {
        match self.view {
            Some(view) => doc
                .pixel_grid()
                .snap_vec(view.origin.to_vec2() - painter.origin()),
            None => Vec2::ZERO,
        }
    }

    fn scale(&self) -> f32 {
        self.view.map_or(1.0, |view| view.scale)
    }

    fn slot(doc: &Document, item: NodeId) -> u8 {
        match doc.arena.get_as::<CanvasItemNode>(item).clip {
            true => CLIPPED,
            false => UNCLIPPED,
        }
    }
}

fn scaled(rect: Rect, scale: f32) -> Rect {
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
        self.host = Some(host);
        self.shift = shift;
        let clipped = doc.enter_space(host, CLIPPED, painter, shift, rect, out);
        let unclipped = doc.enter_space(host, UNCLIPPED, painter, shift, Rect::EVERYTHING, out);
        let visible = rect.translate(-shift);
        let scale = self.scale();
        for item in self.items.iter() {
            let placed = scaled(doc.canvas_item_rect(*item), scale);
            if placed.intersects(visible) {
                let painter = match Self::slot(doc, *item) {
                    CLIPPED => &clipped,
                    _ => &unclipped,
                };
                crate::layout::layout(doc, painter, *item, placed, out);
                continue;
            }
            doc.note_parent(*item);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(host) = self.host else {
            return;
        };
        let clipped = painter.shifted(Some(SpaceId::inside(host, CLIPPED)), self.shift, rect);
        let unclipped = painter.shifted(
            Some(SpaceId::inside(host, UNCLIPPED)),
            self.shift,
            Rect::EVERYTHING,
        );
        for item in self.items.iter() {
            if rects.contains_key(item) {
                let painter = match Self::slot(doc, *item) {
                    CLIPPED => &clipped,
                    _ => &unclipped,
                };
                crate::paint::paint(doc, painter, rects, *item);
            }
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.items.iter().map(ChildItem::node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn kind(&self) -> &'static str {
        "canvas"
    }

    fn detail(&self) -> Option<String> {
        let view = self.view?;
        Some(format!(
            "{:.0},{:.0} at {:.2}",
            view.origin.x, view.origin.y, view.scale
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) struct CanvasItemNode {
    child: Option<NodeId>,
    rect: Rect,
    clip: bool,
}

impl Element for CanvasItemNode {
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

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "canvas item"
    }

    fn detail(&self) -> Option<String> {
        Some(format!(
            "{:.0},{:.0} {:.0}x{:.0}",
            self.rect.min.x,
            self.rect.min.y,
            self.rect.width(),
            self.rect.height()
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
    pub(crate) fn create_canvas(&mut self) -> NodeId {
        self.arena.insert(CanvasNode {
            view: None,
            size: Vec2::ZERO,
            items: ChildList::default(),
            host: None,
            shift: Vec2::ZERO,
        })
    }

    pub(crate) fn set_canvas_view(&mut self, canvas: NodeId, view: Option<CanvasView>) {
        if self.arena.get_as::<CanvasNode>(canvas).view != view {
            self.arena.get_mut_as::<CanvasNode>(canvas).view = view;
        }
    }

    pub(crate) fn set_canvas_size(&mut self, canvas: NodeId, size: Vec2) {
        if self.arena.get_as::<CanvasNode>(canvas).size != size {
            self.arena.get_mut_as::<CanvasNode>(canvas).size = size;
        }
    }

    pub(crate) fn create_canvas_item(&mut self) -> NodeId {
        self.arena.insert(CanvasItemNode {
            child: None,
            rect: Rect::ZERO,
            clip: true,
        })
    }

    pub(crate) fn set_canvas_item_child(&mut self, item: NodeId, child: NodeId) {
        if self.arena.get_as::<CanvasItemNode>(item).child != Some(child) {
            self.arena.get_mut_as::<CanvasItemNode>(item).child = Some(child);
        }
    }

    pub(crate) fn set_canvas_item_rect(&mut self, item: NodeId, rect: Rect) {
        if self.arena.get_as::<CanvasItemNode>(item).rect != rect {
            self.arena.get_mut_as::<CanvasItemNode>(item).rect = rect;
        }
    }

    pub(crate) fn set_canvas_item_clip(&mut self, item: NodeId, clip: bool) {
        if self.arena.get_as::<CanvasItemNode>(item).clip != clip {
            self.arena.get_mut_as::<CanvasItemNode>(item).clip = clip;
        }
    }

    pub(crate) fn canvas_item_rect(&self, item: NodeId) -> Rect {
        self.arena.get_as::<CanvasItemNode>(item).rect
    }
}

pub struct CanvasItem {
    node: NodeId,
}

impl ChildValue for CanvasItem {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

crate::child_type!(CanvasItem);

impl SlotChild for CanvasItem {
    type Stored = NodeId;

    fn store(self, _parent: Option<NodeId>) -> NodeId {
        self.node
    }

    fn stored_node(stored: &NodeId) -> Option<NodeId> {
        Some(*stored)
    }
}

impl NodeSlot for CanvasItem {
    type Host = CanvasNode;
}

#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    children: Children<CanvasItem>,
) -> NodeId {
    let canvas = with_document(Document::create_canvas);
    children.mount(canvas);
    create_effect(move || with_document(|document| document.set_canvas_view(canvas, view.get())));
    create_effect(move || {
        let size = Vec2::new(width.get(), height.get());
        with_document(|document| document.set_canvas_size(canvas, size));
    });
    canvas
}

#[component]
pub fn CanvasItem(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    #[prop(default = true)] clip: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    let item = with_document(|document| {
        let item = document.create_canvas_item();
        if let Some(child) = children {
            document.set_canvas_item_child(item, child);
        }
        item
    });
    create_effect(move || {
        let rect =
            Rect::from_min_size(pos2(x.get(), y.get()), Vec2::new(width.get(), height.get()));
        with_document(|document| document.set_canvas_item_rect(item, rect));
    });
    create_effect(move || {
        with_document(|document| document.set_canvas_item_clip(item, clip.get()))
    });
    CanvasItem { node: item }
}
