use std::any::Any;

use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::input::BackGesture;
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct BackNode {
    child: Option<NodeId>,
    pub(crate) enabled: bool,
    pub(crate) on_back: ClickCallback,
    pub(crate) on_gesture: Callback<BackGesture>,
}

impl Element for BackNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn sizes_itself(&self, doc: &Document, axis: crate::base::list::Direction) -> bool {
        self.child
            .is_some_and(|child| crate::layout::sizes_itself(doc, child, axis))
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
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

    fn paints(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "back handler"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.enabled { "enabled" } else { "disabled" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_back_handler(
        &mut self,
        child: NodeId,
        on_back: ClickCallback,
        on_gesture: Callback<BackGesture>,
    ) -> NodeOf<BackNode> {
        let id = self.arena.insert(BackNode {
            child: Some(child),
            enabled: true,
            on_back,
            on_gesture,
        });
        self.back_handlers.push(id);
        id
    }

    pub fn set_back_handler_enabled(&mut self, handler: NodeOf<BackNode>, enabled: bool) {
        if self.arena.get_as::<BackNode>(handler).enabled == enabled {
            return;
        }
        self.arena.get_mut_as::<BackNode>(handler).enabled = enabled;
        if !enabled {
            self.forget_back_gesture(handler.id());
        }
    }
}
