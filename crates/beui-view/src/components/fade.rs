use crate::reactive::{Child, Prop, create_effect, with_document};
use beui_core::base::Sides;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Fade(#[prop(default = Sides::default())] edges: Prop<Sides>, children: Child) -> NodeId {
    let fade = with_document(|document| document.create_fade(children));
    create_effect(move || {
        let edges = edges.get();
        with_document(|document| document.set_fade(fade, edges));
    });
    fade.id()
}
