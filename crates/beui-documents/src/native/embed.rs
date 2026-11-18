use crate::NodeId;
use beui_core::base::embed::EmbedSlot;
use beui_core::current::with_document;
use beui_macros::component;
use beui_view::reactive::{Child, Get, Prop, create_effect};

#[component]
pub fn Embed(
    slot: EmbedSlot,
    #[prop(default = true)] punch: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 0.0)] radius: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
    let state = slot.0.value();
    let embed = with_document(move |document| {
        let embed = document.create_embed(state);
        if let Some(child) = children {
            document.set_embed_child(embed, child);
        }
        embed
    });
    slot.0.with(|held| held.node.set(Some(embed.id())));
    create_effect(move || {
        let punch = punch.get();
        with_document(|document| document.set_embed_punch(embed, punch));
    });
    create_effect(move || {
        let rotation = rotation.get();
        with_document(|document| document.set_embed_rotation(embed, rotation));
    });
    create_effect(move || {
        let radius = radius.get();
        with_document(|document| document.set_embed_radius(embed, radius));
    });
    embed.id()
}
