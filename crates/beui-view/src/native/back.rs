use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::input::BackGesture;
use beui_core::node::NodeId;

pub fn back_handler(
    enabled: Prop<bool>,
    on_back: ClickCallback,
    on_gesture: Callback<BackGesture>,
    children: Child
) -> NodeId {
    let handler =
        with_document(|document| document.create_back_handler(children, on_back, on_gesture));
    create_effect(move || {
        let enabled = enabled.get();
        with_document(|document| document.set_back_handler_enabled(handler, enabled));
    });
    handler.id()
}
