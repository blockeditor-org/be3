use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use beui_macros::component;

use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Child, Prop, create_effect, with_document};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbedPlacement {
    pub rect: Rect,
    pub clip: Rect,
}

#[derive(Default)]
pub(crate) struct EmbedState {
    node: Cell<Option<NodeId>>,
    placement: Cell<Option<EmbedPlacement>>,
}

#[derive(Clone, Default)]
pub struct EmbedSlot(Rc<EmbedState>);

impl EmbedSlot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn node(&self) -> Option<NodeId> {
        self.0.node.get()
    }

    pub fn placement(&self) -> Option<EmbedPlacement> {
        self.0.placement.get()
    }
}

pub(crate) struct EmbedNode {
    child: Option<NodeId>,
    width: Option<f32>,
    height: Option<f32>,
    punch: bool,
    rotation: f32,
    state: Rc<EmbedState>,
}

impl Element for EmbedNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let inner = match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        };
        vec2(
            self.width.unwrap_or(inner.x),
            self.height.unwrap_or(inner.y),
        )
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        let grid = doc.pixel_grid();
        let placed = grid.snap_rect(rect);
        self.state.placement.set(Some(EmbedPlacement {
            rect: placed,
            clip: grid.snap_rect(painter.clip_rect()).intersect(placed),
        }));
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, rect: Rect) {
        if self.punch {
            painter
                .rotated(rect.center(), self.rotation)
                .punch(rect, 0.0);
        }
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        self.punch
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "embed"
    }

    fn detail(&self) -> Option<String> {
        let placement = self.state.placement.get()?;
        Some(format!(
            "{:.0}x{:.0}",
            placement.rect.width(),
            placement.rect.height()
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
    pub(crate) fn create_embed(&mut self, state: Rc<EmbedState>) -> NodeId {
        self.arena.insert(EmbedNode {
            child: None,
            width: None,
            height: None,
            punch: false,
            rotation: 0.0,
            state,
        })
    }

    pub(crate) fn set_embed_child(&mut self, embed: NodeId, child: NodeId) {
        if self.arena.get_as::<EmbedNode>(embed).child != Some(child) {
            self.arena.get_mut_as::<EmbedNode>(embed).child = Some(child);
        }
    }

    pub(crate) fn set_embed_punch(&mut self, embed: NodeId, punch: bool) {
        if self.arena.get_as::<EmbedNode>(embed).punch != punch {
            self.arena.get_mut_as::<EmbedNode>(embed).punch = punch;
        }
    }

    pub(crate) fn set_embed_rotation(&mut self, embed: NodeId, rotation: f32) {
        if self.arena.get_as::<EmbedNode>(embed).rotation != rotation {
            self.arena.get_mut_as::<EmbedNode>(embed).rotation = rotation;
        }
    }

    pub(crate) fn set_embed_size(
        &mut self,
        embed: NodeId,
        width: Option<f32>,
        height: Option<f32>,
    ) {
        let node = self.arena.get_as::<EmbedNode>(embed);
        if node.width == width && node.height == height {
            return;
        }
        let node = self.arena.get_mut_as::<EmbedNode>(embed);
        node.width = width;
        node.height = height;
    }
}

#[component]
pub fn Embed(
    slot: EmbedSlot,
    width: Option<Prop<f32>>,
    height: Option<Prop<f32>>,
    #[prop(default = true)] punch: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
    let state = Rc::clone(&slot.0);
    let embed = with_document(move |document| {
        let embed = document.create_embed(state);
        if let Some(child) = children {
            document.set_embed_child(embed, child);
        }
        embed
    });
    slot.0.node.set(Some(embed));
    create_effect(move || {
        let width = width.as_ref().map(Prop::get);
        let height = height.as_ref().map(Prop::get);
        with_document(|document| document.set_embed_size(embed, width, height));
    });
    create_effect(move || {
        let punch = punch.get();
        with_document(|document| document.set_embed_punch(embed, punch));
    });
    create_effect(move || {
        let rotation = rotation.get();
        with_document(|document| document.set_embed_rotation(embed, rotation));
    });
    embed
}
