use crate::reactive::{Child, ClickCallback, Prop, create_effect, with_document};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn BackHandler(
    #[prop(default = true)] enabled: Prop<bool>,
    on_back: ClickCallback,
    children: Child,
) -> NodeId {
    let handler = with_document(|document| document.create_back_handler(children, on_back));
    create_effect(move || {
        let enabled = enabled.get();
        with_document(|document| document.set_back_handler_enabled(handler, enabled));
    });
    handler
}
