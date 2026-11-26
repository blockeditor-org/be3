use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::component;
use crate::types::CanvasArrangement;
use crate::types::CanvasView;
use crate::types::Vec2;
pub use crate::types::tree::CanvasItem;

/// Items placed in world coordinates through `view` (`origin + point * scale`). The view's
/// origin is in the frame's points, a camera that holds still however the layout moves the
/// canvas; with `local_view` it is in the canvas's own points, so the canvas's content moves
/// with it as any node's does (a camera fitted to a canvas inside a scrolled form).
#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    #[prop(default = false)] scrolls_across: bool,
    #[prop(default = false)] local_view: bool,
    #[prop(default = None)] arrangement: Prop<Option<CanvasArrangement>>,
    on_reveal: Callback<Vec2>,
    children: Children<CanvasItem>,
) -> NodeId {
    crate::core::canvas(
        view,
        width,
        height,
        scrolls_across,
        local_view,
        arrangement,
        on_reveal,
        children,
    )
}

#[component]
pub fn CanvasItem(
    #[prop(default = 0.0)] x: Prop<f32>,
    #[prop(default = 0.0)] y: Prop<f32>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    arranged: Option<u64>,
    #[prop(default = true)] clip: Prop<bool>,
    #[prop(default = true)] pointer: Prop<bool>,
    #[prop(default = false)] scales: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    crate::core::canvas_item(
        x, y, width, height, arranged, clip, pointer, scales, children,
    )
}
