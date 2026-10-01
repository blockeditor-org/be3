use std::any::Any;
use std::rc::Rc;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;

pub struct DrawingNode {
    draw: Option<Draw>,
}

impl Element for DrawingNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        let Some(draw) = self.draw.as_ref() else {
            return;
        };
        draw(painter, rect);
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        _children: &mut Vec<NodeId>,
    ) {
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

impl Document {
    pub fn create_drawing(&mut self) -> NodeOf<DrawingNode> {
        self.arena.insert(DrawingNode { draw: None })
    }

    pub fn set_drawing(&mut self, drawing: NodeOf<DrawingNode>, draw: Draw) {
        let held = self.arena.get_as::<DrawingNode>(drawing).draw.as_ref();
        if held.is_some_and(|held| Rc::ptr_eq(held, &draw)) {
            return;
        }
        self.arena.paint_mut_as::<DrawingNode>(drawing).draw = Some(draw);
    }
}
