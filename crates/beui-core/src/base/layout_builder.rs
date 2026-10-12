use std::any::Any;

use ::reactive::{ReadSignal, WriteSignal};

use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct LayoutBuilderNode {
    child: Option<NodeId>,
    size: ReadSignal<Vec2>,
    set_size: WriteSignal<Vec2>,
}

impl LayoutBuilderNode {
    fn offered(&self, available: Vec2) -> Option<(WriteSignal<Vec2>, Vec2)> {
        let held = self.size.get_untracked();
        let offered = vec2(
            match available.x.is_finite() {
                true => available.x,
                false => held.x,
            },
            match available.y.is_finite() {
                true => available.y,
                false => held.y,
            },
        );
        (offered != held).then(|| (self.set_size.clone(), offered))
    }
}

impl Element for LayoutBuilderNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
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

    fn passes_scroll_anchor(&self) -> bool {
        true
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "layout builder"
    }

    fn detail(&self) -> Option<String> {
        let size = self.size.get_untracked();
        Some(format!("{:.0}x{:.0}", size.x, size.y))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_layout_builder(
        &mut self,
        size: ReadSignal<Vec2>,
        set_size: WriteSignal<Vec2>,
    ) -> NodeOf<LayoutBuilderNode> {
        self.arena.insert(LayoutBuilderNode {
            child: None,
            size,
            set_size,
        })
    }

    pub fn set_layout_builder_child(&mut self, builder: NodeOf<LayoutBuilderNode>, child: NodeId) {
        if self.arena.get_as(builder).child != Some(child) {
            self.arena.get_mut_as(builder).child = Some(child);
        }
    }

    pub(crate) fn offer_constraint(&mut self, id: NodeId, available: Vec2) -> bool {
        let Some(builder) = self.arena.kind_of::<LayoutBuilderNode>(id) else {
            return false;
        };
        if let Some((set_size, size)) = self.arena.get_as(builder).offered(available) {
            ::reactive::settle(move || set_size.set(size));
        }
        true
    }
}
