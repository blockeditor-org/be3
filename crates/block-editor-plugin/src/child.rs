use beui::NodeId;
use beui::reactive::{
    Callback, Embed, EmbedSlot, IntoProp, Prop, ReadSignal, Render, component, create_memo, view,
};
use block_plugin_api::{ChildLayer, ChildMode};

use crate::{ChildState, ChildTarget, Editor};

#[derive(Clone)]
pub struct ChildHandle {
    pub state: ReadSignal<ChildState>,
}

#[component]
pub fn ChildBlock(
    editor: Editor,
    block: Prop<Option<ChildTarget>>,
    #[prop(default = ChildMode::Passive)] mode: Prop<ChildMode>,
    #[prop(default = ChildLayer::Below)] layer: Prop<ChildLayer>,
    #[prop(default = false)] own_frame: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = None)] intrinsic: Prop<Option<beui::Vec2>>,
    on_state: Callback<ChildState>,
    #[prop(children)] content: Option<Render<ChildHandle>>,
) -> NodeId {
    let slot = EmbedSlot::new();
    let layer = create_memo(move || layer.get());
    let punch = layer
        .clone()
        .into_prop()
        .map(|layer| matches!(layer, ChildLayer::Below));
    let turn = create_memo(move || rotation.get());
    let state = editor.register_child(
        slot.clone(),
        block,
        mode,
        layer.into_prop(),
        own_frame,
        turn.clone().into_prop(),
        opacity,
        intrinsic,
        on_state,
    );
    let handle = ChildHandle { state };
    let children = content.map(|content| content.call(handle));
    view! {
        <Embed slot={slot} punch={punch} rotation={turn} children={children} />
    }
}
