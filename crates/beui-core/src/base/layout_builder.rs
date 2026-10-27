use std::any::Any;
use std::cell::Cell;

use ::reactive::{Get, ReadSignal, ScopeContext, Set, Signal};

use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

enum Builds {
    Size(Signal<Vec2>),
    Placement(ReadSignal<Rect>),
}

pub struct LayoutBuilderNode {
    child: Option<NodeId>,
    content: Option<ScopeContext>,
    builds: Builds,
    offered: Cell<Vec2>,
}

impl LayoutBuilderNode {
    fn given(&self, size: Vec2) -> Option<(Signal<Vec2>, Vec2)> {
        let Builds::Size(held_size) = &self.builds else {
            return None;
        };
        let offered = self.offered.get();
        let held = held_size.get_untracked();
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
        (given != held).then_some((*held_size, given))
    }
}

impl LayoutBuilderNode {
    fn rebuild(&self, doc: &mut Document, painter: &Painter, held: Signal<Vec2>, size: Vec2) {
        let builder = doc
            .laying_out()
            .expect("a layout builder is laid out as itself");
        let content = self.content.clone();
        doc.build_within(builder, move || {
            held.set(size);
            if let Some(content) = content {
                content.flush_owned();
            }
        });
        let (Some(child), offered) = (self.child, self.offered.get()) else {
            return;
        };
        if let Some(measured) = doc.measured(builder, offered)
            && crate::layout::measure(doc, painter, child, offered) != measured
        {
            doc.invalidate_after_layout(builder);
        }
    }
}

impl Element for LayoutBuilderNode {
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

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some((held, size)) = self.given(rect.size()) {
            self.rebuild(doc, painter, held, size);
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

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        if let Some(child) = self.child {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "layout builder"
    }

    fn detail(&self) -> Option<String> {
        Some(match &self.builds {
            Builds::Size(size) => {
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
    pub fn create_layout_builder(&mut self, size: Signal<Vec2>) -> NodeOf<LayoutBuilderNode> {
        self.arena.insert(LayoutBuilderNode {
            child: None,
            content: None,
            builds: Builds::Size(size),
            offered: Cell::new(Vec2::ZERO),
        })
    }

    pub fn create_placement_builder(&mut self) -> (NodeOf<LayoutBuilderNode>, ReadSignal<Rect>) {
        let watched = ::reactive::OwnedSignal::new(Rect::ZERO);
        let placed = watched.read_only();
        let builder = self.arena.insert(LayoutBuilderNode {
            child: None,
            content: None,
            builds: Builds::Placement(placed),
            offered: Cell::new(Vec2::ZERO),
        });
        self.watch_placement_with(builder.id(), watched);
        (builder, placed)
    }

    pub fn set_layout_builder_child(&mut self, builder: NodeOf<LayoutBuilderNode>, child: NodeId) {
        if self.arena.get_as(builder).child != Some(child) {
            self.arena.get_mut_as(builder).child = Some(child);
        }
    }

    pub fn set_layout_builder_content(
        &mut self,
        builder: NodeOf<LayoutBuilderNode>,
        content: ScopeContext,
    ) {
        self.arena.touch_mut_as(builder).content = Some(content);
    }

    pub(crate) fn note_offer(&self, id: NodeId, available: Vec2) {
        if let Some(builder) = self.arena.kind_of::<LayoutBuilderNode>(id) {
            self.arena.get_as(builder).offered.set(available);
        }
    }
}
