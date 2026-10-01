use crate::reactive::{Callback, Children, Prop, create_effect, with_document};
use beui_core::base::list::Direction;
use beui_core::base::offset::{OffsetNode, ScrollPosition};
use beui_core::node::{NodeId, NodeOf};
use beui_macros::component;

#[component]
pub fn Offset(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let node = create_offset(direction, on_change);
    with_document(|document| document.set_offset_fits(node, fit));
    children.mount(node);
    create_effect(move || with_document(|document| document.set_offset_value(node, offset.get())));
    create_effect(move || {
        let index = reveal.get();
        let Some(index) = index else {
            return;
        };
        with_document(|document| document.reveal_offset_index(node, index));
    });
    node.id()
}

fn create_offset(
    direction: Prop<Direction>,
    on_change: Callback<ScrollPosition>,
) -> NodeOf<OffsetNode> {
    let offset = with_document(|document| {
        let offset = document.create_offset();
        document.set_offset_on_change(offset, move |position| on_change.call(position));
        offset
    });
    create_effect(move || {
        with_document(|document| document.set_offset_direction(offset, direction.get()))
    });
    offset
}
