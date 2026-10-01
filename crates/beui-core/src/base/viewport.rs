use std::any::Any;

use crate::document::Document;
use crate::drawing::Drawing;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct ViewportNode {
    drawing: Option<Drawing>,
}

fn bounded(available: f32) -> f32 {
    match available.is_finite() {
        true => available.max(0.0),
        false => 0.0,
    }
}

impl Element for ViewportNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, available: Vec2) -> Vec2 {
        vec2(bounded(available.x), bounded(available.y))
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        if let Some(drawing) = self.drawing.as_ref() {
            painter.drawing(rect, drawing);
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
        _children: &mut Vec<NodeId>,
    ) {
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str {
        "viewport"
    }

    fn detail(&self) -> Option<String> {
        self.drawing.as_ref().map(|_| "drawn".to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_viewport(&mut self) -> NodeOf<ViewportNode> {
        self.arena.insert(ViewportNode { drawing: None })
    }

    pub fn set_viewport_drawing(
        &mut self,
        viewport: NodeOf<ViewportNode>,
        drawing: Option<Drawing>,
    ) {
        if self.arena.get_as::<ViewportNode>(viewport).drawing != drawing {
            self.arena.paint_mut_as::<ViewportNode>(viewport).drawing = drawing;
        }
    }
}
