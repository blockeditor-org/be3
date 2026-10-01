use crate::reactive::{Callback, Child, ClickCallback, Prop, create_effect, with_document};
use beui_core::input::BackGesture;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn BackHandler(
    #[prop(default = true)] enabled: Prop<bool>,
    #[prop(default = ClickCallback::default())] on_back: ClickCallback,
    on_gesture: Callback<BackGesture>,
    children: Child,
) -> NodeId {
    let handler =
        with_document(|document| document.create_back_handler(children, on_back, on_gesture));
    create_effect(move || {
        let enabled = enabled.get();
        with_document(|document| document.set_back_handler_enabled(handler, enabled));
    });
    handler.id()
}
