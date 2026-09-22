use std::any::Any;
use std::rc::Rc;

use beui_macros::component;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Prop, create_effect, with_document};

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;

pub(crate) struct DrawingNode {
    draw: Option<Draw>,
}

impl Element for DrawingNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _rect: Rect,
        _out: &mut NodeMap<Rect>,
    ) {
    }

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, rect: Rect) {
        let Some(draw) = self.draw.as_ref() else {
            return;
        };
        draw(painter, rect);
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str {
        "drawing"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[component]
pub fn Drawing(draw: Prop<Draw>) -> NodeId {
    let drawing = with_document(Document::create_drawing);
    create_effect(move || {
        let draw = draw.get();
        with_document(|document| document.set_drawing(drawing, draw));
    });
    drawing
}

impl Document {
    pub(crate) fn create_drawing(&mut self) -> NodeId {
        self.arena.insert(DrawingNode { draw: None })
    }

    pub(crate) fn set_drawing(&mut self, drawing: NodeId, draw: Draw) {
        let held = self.arena.get_as::<DrawingNode>(drawing).draw.as_ref();
        if held.is_some_and(|held| Rc::ptr_eq(held, &draw)) {
            return;
        }
        self.arena.get_mut_as::<DrawingNode>(drawing).draw = Some(draw);
    }
}
