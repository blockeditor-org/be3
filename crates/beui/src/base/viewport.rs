use std::any::Any;

use beui_macros::component;

use crate::document::Document;
use crate::drawing::Drawing;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Prop, create_effect, with_document};

pub(crate) struct ViewportNode {
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

    fn layout(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _rect: Rect,
        _out: &mut NodeMap<Rect>,
    ) {
    }

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, rect: Rect) {
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

#[component]
pub fn Viewport(drawing: Prop<Option<Drawing>>) -> NodeId {
    let viewport = with_document(Document::create_viewport);
    create_effect(move || {
        let drawing = drawing.get();
        with_document(|document| document.set_viewport_drawing(viewport, drawing));
    });
    viewport
}

impl Document {
    pub(crate) fn create_viewport(&mut self) -> NodeId {
        self.arena.insert(ViewportNode { drawing: None })
    }

    pub(crate) fn set_viewport_drawing(&mut self, viewport: NodeId, drawing: Option<Drawing>) {
        if self.arena.get_as::<ViewportNode>(viewport).drawing != drawing {
            self.arena.get_mut_as::<ViewportNode>(viewport).drawing = drawing;
        }
    }
}
