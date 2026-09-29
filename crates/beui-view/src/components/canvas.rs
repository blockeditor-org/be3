use crate::reactive::{
    BuildsNode, Child, ChildValue, Children, NodeSlot, Prop, Scope, SlotChild, create_effect,
    with_document,
};
use beui_core::base::canvas::{CanvasNode, CanvasView};
use beui_core::document::Document;
use beui_core::geometry::{Rect, Vec2, pos2};
use beui_core::node::NodeId;
use beui_macros::component;

pub struct CanvasItem {
    node: NodeId,
}

impl BuildsNode for CanvasItem {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for CanvasItem {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

crate::child_type!(CanvasItem);

impl SlotChild for CanvasItem {
    type Stored = NodeId;

    fn store(self, _parent: Option<NodeId>) -> NodeId {
        self.node
    }

    fn stored_node(stored: &NodeId) -> Option<NodeId> {
        Some(*stored)
    }
}

impl NodeSlot for CanvasItem {
    type Host = CanvasNode;
}

#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    children: Children<CanvasItem>,
) -> NodeId {
    let canvas = with_document(Document::create_canvas);
    children.mount(canvas);
    create_effect(move || with_document(|document| document.set_canvas_view(canvas, view.get())));
    create_effect(move || {
        let size = Vec2::new(width.get(), height.get());
        with_document(|document| document.set_canvas_size(canvas, size));
    });
    canvas
}

#[component]
pub fn CanvasItem(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    #[prop(default = true)] clip: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    let item = with_document(|document| {
        let item = document.create_canvas_item();
        if let Some(child) = children {
            document.set_canvas_item_child(item, child);
        }
        item
    });
    create_effect(move || {
        let rect =
            Rect::from_min_size(pos2(x.get(), y.get()), Vec2::new(width.get(), height.get()));
        with_document(|document| document.set_canvas_item_rect(item, rect));
    });
    create_effect(move || {
        with_document(|document| document.set_canvas_item_clip(item, clip.get()))
    });
    CanvasItem { node: item }
}
