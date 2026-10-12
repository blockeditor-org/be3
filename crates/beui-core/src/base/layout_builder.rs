use std::any::Any;
use std::cell::Cell;

use ::reactive::{ReadSignal, WriteSignal};

use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

enum Builds {
    Size(ReadSignal<Vec2>, WriteSignal<Vec2>),
    Placement(ReadSignal<Rect>),
}

pub struct LayoutBuilderNode {
    child: Option<NodeId>,
    builds: Builds,
    offered: Cell<Vec2>,
}

impl LayoutBuilderNode {
    fn given(&self, size: Vec2) -> Option<(WriteSignal<Vec2>, Vec2)> {
        let Builds::Size(held, set_size) = &self.builds else {
            return None;
        };
        let offered = self.offered.get();
        let held = held.get_untracked();
        let given = vec2(
            match offered.x.is_finite() {
                true => size.x,
                false => held.x,
            },
            match offered.y.is_finite() {
                true => size.y,
                false => held.y,
            },
        );
        (given != held).then(|| (set_size.clone(), given))
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
        if let Some((set_size, size)) = self.given(rect.size()) {
            doc.report_after_layout(move || set_size.set(size));
        }
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
        Some(match &self.builds {
            Builds::Size(size, _) => {
                let size = size.get_untracked();
                format!("{:.0}x{:.0}", size.x, size.y)
            }
            Builds::Placement(placed) => {
                let rect = placed.get_untracked();
                format!(
                    "{:.0}x{:.0} at {:.0}, {:.0}",
                    rect.width(),
                    rect.height(),
                    rect.min.x,
                    rect.min.y
                )
            }
        })
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
            builds: Builds::Size(size, set_size),
            offered: Cell::new(Vec2::ZERO),
        })
    }

    pub fn create_placement_builder(
        &mut self,
        placed: ReadSignal<Rect>,
        set_placed: WriteSignal<Rect>,
    ) -> NodeOf<LayoutBuilderNode> {
        let builder = self.arena.insert(LayoutBuilderNode {
            child: None,
            builds: Builds::Placement(placed.clone()),
            offered: Cell::new(Vec2::ZERO),
        });
        self.follow_placement(builder.id(), placed, set_placed);
        builder
    }

    pub fn set_layout_builder_child(&mut self, builder: NodeOf<LayoutBuilderNode>, child: NodeId) {
        if self.arena.get_as(builder).child != Some(child) {
            self.arena.get_mut_as(builder).child = Some(child);
        }
    }

    pub(crate) fn note_offer(&self, id: NodeId, available: Vec2) {
        if let Some(builder) = self.arena.kind_of::<LayoutBuilderNode>(id) {
            self.arena.get_as(builder).offered.set(available);
        }
    }
}
