use std::rc::Rc;

use crate::reactive::{Child, Prop, create_effect, with_document};
use beui_core::base::embed::EmbedSlot;
use beui_core::node::NodeId;
use beui_macros::component;

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
    slot.0.node.set(Some(embed.id()));
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
    embed.id()
}
