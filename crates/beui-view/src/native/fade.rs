use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::base::Sides;
use beui_core::node::NodeId;

pub fn fade(
    edges: Prop<Sides>,
    opacity: Prop<f32>,
    children: Child
) -> NodeId {
    let fade = with_document(|document| document.create_fade(children));
    create_effect(move || {
        let edges = edges.get();
        with_document(|document| document.set_fade(fade, edges));
    });
    create_effect(move || {
        let opacity = opacity.get();
        with_document(|document| document.set_fade_opacity(fade, opacity));
    });
    fade.id()
}
