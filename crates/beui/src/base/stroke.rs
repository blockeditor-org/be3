use std::any::Any;

use beui_macros::component;

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::node::{Element, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Prop, create_effect, with_document};

pub(crate) struct StrokeNode {
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

    fn layout(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _rect: Rect,
        _out: &mut NodeMap<Rect>,
    ) {
    }

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, rect: Rect) {
        let origin = rect.min.to_vec2();
        painter.line(self.from + origin, self.to + origin, self.width, self.color);
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

#[component]
pub fn Stroke(
    from: Prop<Pos2>,
    to: Prop<Pos2>,
    #[prop(default = 1.0)] width: Prop<f32>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
) -> NodeId {
    let stroke = with_document(Document::create_stroke);
    create_effect(move || {
        let (from, to) = (from.get(), to.get());
        with_document(|document| document.set_stroke_ends(stroke, from, to));
    });
    create_effect(move || {
        let width = width.get();
        with_document(|document| document.set_stroke_width(stroke, width));
    });
    create_effect(move || {
        let color = color.get();
        with_document(|document| document.set_stroke_color(stroke, color));
    });
    stroke
}

impl Document {
    pub(crate) fn create_stroke(&mut self) -> NodeId {
        self.arena.insert(StrokeNode {
            from: Pos2::ZERO,
            to: Pos2::ZERO,
            width: 1.0,
            color: Color32::WHITE,
        })
    }

    pub(crate) fn set_stroke_ends(&mut self, stroke: NodeId, from: Pos2, to: Pos2) {
        let node = self.arena.get_as::<StrokeNode>(stroke);
        if node.from != from || node.to != to {
            let node = self.arena.get_mut_as::<StrokeNode>(stroke);
            node.from = from;
            node.to = to;
        }
    }

    pub(crate) fn set_stroke_width(&mut self, stroke: NodeId, width: f32) {
        if self.arena.get_as::<StrokeNode>(stroke).width != width {
            self.arena.get_mut_as::<StrokeNode>(stroke).width = width;
        }
    }

    pub(crate) fn set_stroke_color(&mut self, stroke: NodeId, color: Color32) {
        if self.arena.get_as::<StrokeNode>(stroke).color != color {
            self.arena.get_mut_as::<StrokeNode>(stroke).color = color;
        }
    }
}
