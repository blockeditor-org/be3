use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::base::frame::Align2;
use beui_core::base::frame::Extent;
use beui_core::base::frame::Outline;
use beui_core::base::frame::Sides;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_core::painter::Corners;
use beui_macros::component;

#[component]
pub fn Frame(
    #[prop(default = Extent::INTRINSIC)] width: Prop<Extent>,
    #[prop(default = Extent::INTRINSIC)] height: Prop<Extent>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = Sides::ZERO)] padding: Prop<Sides>,
    #[prop(default = Align2::STRETCH)] align: Prop<Align2>,
    #[prop(default = Color32::TRANSPARENT)] fill: Prop<Color32>,
    #[prop(default = Corners::ZERO)] radius: Prop<Corners>,
    #[prop(default = None)] outline: Prop<Option<Outline>>,
    #[prop(default = 0.0)] pointer: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
    crate::core::frame(
        width,
        height,
        aspect_ratio,
        padding,
        align,
        fill,
        radius,
        outline,
        pointer,
        children,
    )
}
