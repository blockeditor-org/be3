use std::any::Any;

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, Rects, NodeOf};
use crate::painter::Painter;

pub struct StrokeNode {
    from: Pos2,
    to: Pos2,
    width: f32,
    color: Color32,
}

impl StrokeNode {
    fn extent(&self) -> Vec2 {
        Vec2::new(
            self.from.x.max(self.to.x) + self.width / 2.0,
            self.from.y.max(self.to.y) + self.width / 2.0,
        )
    }
}

impl Element for StrokeNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        self.extent()
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        let origin = rect.min.to_vec2();
        painter.line(self.from + origin, self.to + origin, self.width, self.color);
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
        "stroke"
    }

    fn detail(&self) -> Option<String> {
        Some(format!(
            "({}, {}) to ({}, {})",
            self.from.x, self.from.y, self.to.x, self.to.y
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
    pub fn create_stroke(&mut self) -> NodeOf<StrokeNode> {
        self.arena.insert(StrokeNode {
            from: Pos2::ZERO,
            to: Pos2::ZERO,
            width: 1.0,
            color: Color32::WHITE,
        })
    }

    pub fn set_stroke_ends(&mut self, stroke: NodeOf<StrokeNode>, from: Pos2, to: Pos2) {
        let node = self.arena.get_as::<StrokeNode>(stroke);
        if node.from != from || node.to != to {
            let node = self.arena.get_mut_as::<StrokeNode>(stroke);
            node.from = from;
            node.to = to;
        }
    }

    pub fn set_stroke_width(&mut self, stroke: NodeOf<StrokeNode>, width: f32) {
        if self.arena.get_as::<StrokeNode>(stroke).width != width {
            self.arena.get_mut_as::<StrokeNode>(stroke).width = width;
        }
    }

    pub fn set_stroke_color(&mut self, stroke: NodeOf<StrokeNode>, color: Color32) {
        if self.arena.get_as::<StrokeNode>(stroke).color != color {
            self.arena.paint_mut_as::<StrokeNode>(stroke).color = color;
        }
    }
}
