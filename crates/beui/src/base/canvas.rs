use std::any::Any;
use std::collections::HashMap;

use beui_macros::component;

use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::node::{Element, InteractInput, NodeId};
use crate::painter::Painter;
use crate::reactive::{Child, Children, Prop, create_effect, with_document};

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

pub(crate) struct CanvasNode {
    view: Option<CanvasView>,
    items: Vec<NodeId>,
}

impl CanvasNode {
    fn placement(&self, rect: Rect) -> CanvasView {
        self.view.unwrap_or(CanvasView {
            origin: rect.min,
            scale: 1.0,
        })
    }
}

impl Element for CanvasNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let view = self.placement(rect);
        for item in &self.items {
            let placed = view.rect_to_screen(doc.canvas_item_rect(*item));
            if placed.intersects(rect) {
                crate::layout::layout(doc, painter, *item, placed, out);
            }
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(rect);
        for item in &self.items {
            if rects.contains_key(item) {
                crate::paint::paint(doc, &clipped, rects, *item);
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
    ) -> Vec<NodeId> {
        self.items.clone()
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.clone()
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
}

impl Element for CanvasItemNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        self.rect.size()
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
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
    ) -> Vec<NodeId> {
        self.child.into_iter().collect()
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
            items: Vec::new(),
        })
    }

    pub(crate) fn set_canvas_view(&mut self, canvas: NodeId, view: Option<CanvasView>) {
        if self.arena.get_as::<CanvasNode>(canvas).view != view {
            self.arena.get_mut_as::<CanvasNode>(canvas).view = view;
        }
    }

    pub(crate) fn append_canvas_item(&mut self, canvas: NodeId, item: NodeId) {
        self.arena.get_mut_as::<CanvasNode>(canvas).items.push(item);
    }

    pub(crate) fn create_canvas_item(&mut self) -> NodeId {
        self.arena.insert(CanvasItemNode {
            child: None,
            rect: Rect::ZERO,
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

    pub(crate) fn canvas_item_rect(&self, item: NodeId) -> Rect {
        self.arena.get_as::<CanvasItemNode>(item).rect
    }
}

#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    children: Children,
) -> NodeId {
    let canvas = with_document(Document::create_canvas);
    with_document(|document| {
        for (item, _) in children.into_items() {
            document.append_canvas_item(canvas, item);
        }
    });
    create_effect(move || with_document(|document| document.set_canvas_view(canvas, view.get())));
    canvas
}

#[component]
pub fn CanvasItem(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
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
    item
}
