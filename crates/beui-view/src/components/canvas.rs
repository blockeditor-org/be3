pub use crate::core::canvas::CanvasItem;
use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::CanvasView;
use crate::types::Vec2;
use beui_macros::component;

#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    #[prop(default = false)] scrolls_across: bool,
    on_reveal: Callback<Vec2>,
    children: Children<CanvasItem>,
) -> NodeId {
    crate::core::canvas(view, width, height, scrolls_across, on_reveal, children)
}

#[component]
pub fn CanvasItem(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    #[prop(default = true)] clip: Prop<bool>,
    #[prop(default = true)] pointer: Prop<bool>,
    #[prop(default = false)] scales: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    crate::core::canvas_item(x, y, width, height, clip, pointer, scales, children)
}
