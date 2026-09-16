use beui::NodeId;
use beui::reactive::{Callback, Embed, EmbedSlot, Prop, ReadSignal, Render, component, view};
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
    on_state: Callback<ChildState>,
    #[prop(children)] content: Option<Render<ChildHandle>>,
) -> NodeId {
    let slot = EmbedSlot::new();
    let state = editor.register_child(slot.clone(), block, mode, layer, on_state);
    let handle = ChildHandle { state };
    let children = content.map(|content| content.call(handle));
    view! {
        <Embed slot={slot} children={children} />
    }
}
