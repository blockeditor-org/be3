use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::base::embed::EmbedSlot;
use beui_core::node::NodeId;

pub fn embed(
    slot: EmbedSlot,
    punch: Prop<bool>,
    rotation: Prop<f32>,
    children: Option<Child>
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
    embed.id()
}
