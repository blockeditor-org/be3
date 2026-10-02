use crate::reactive::{Child, Prop, create_effect, with_document};
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Shift(by: Prop<Vec2>, children: Child) -> NodeId {
    let shift = with_document(|document| document.create_shift(children));
    create_effect(move || {
        let by = by.get();
        with_document(|document| document.set_shift(shift, by));
    });
    shift.id()
}
