use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::base::list::Direction;
use beui_core::base::offset::ScrollPosition;
use beui_core::node::NodeId;

pub fn offset(
    offset: Prop<f32>,
    direction: Prop<Direction>,
    fit: bool,
    follow_end: Prop<bool>,
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
        let direction = direction.get();
        with_document(|document| document.set_offset_direction(node, direction))
    });
    create_effect(move || {
        let follows = follow_end.get();
        with_document(|document| document.set_offset_follows_end(node, follows))
    });
    create_effect(move || {
        let offset = offset.get();
        with_document(|document| document.set_offset_value(node, offset))
    });
    node.id()
}
