use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::Prop;
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
    crate::core::back_handler(enabled, on_back, on_gesture, children)
}
