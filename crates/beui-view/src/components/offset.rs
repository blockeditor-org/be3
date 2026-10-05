use crate::reactive::{Callback, Child, Prop, create_effect, with_document};
use beui_core::base::list::Direction;
use beui_core::base::offset::ScrollPosition;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Offset(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    on_change: Callback<ScrollPosition>,
    children: Child,
) -> NodeId {
    let node = with_document(|document| {
        let node = document.create_offset(children);
        document.set_offset_fits(node, fit);
        document.set_offset_on_change(node, move |position| on_change.call(position));
        node
    });
    create_effect(move || {
        with_document(|document| document.set_offset_direction(node, direction.get()))
    });
    create_effect(move || with_document(|document| document.set_offset_value(node, offset.get())));
    node.id()
}
