use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;

pub fn shift(
    by: Prop<Vec2>,
    children: Child
) -> NodeId {
    let shift = with_document(|document| document.create_shift(children));
    create_effect(move || {
        let by = by.get();
        with_document(|document| document.set_shift(shift, by));
    });
    shift.id()
}
