use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Align2;
use crate::types::Color;
use crate::types::Corners;
use crate::types::Extent;
use crate::types::Outline;
use crate::types::Sides;
use crate::types::Vec2;
use beui_macros::component;

#[component]
pub fn Frame(
    #[prop(default = Extent::INTRINSIC)] width: Prop<Extent>,
    #[prop(default = Extent::INTRINSIC)] height: Prop<Extent>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = Sides::ZERO)] padding: Prop<Sides>,
    #[prop(default = Align2::STRETCH)] align: Prop<Align2>,
    #[prop(default = Color::TRANSPARENT)] fill: Prop<Color>,
    #[prop(default = Corners::ZERO)] radius: Prop<Corners>,
    #[prop(default = None)] outline: Prop<Option<Outline>>,
    #[prop(default = Vec2::ZERO)] translate: Prop<Vec2>,
    #[prop(default = 0.0)] rotate: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = Sides::ZERO)] fade_edges: Prop<Sides>,
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
        translate,
        rotate,
        opacity,
        fade_edges,
        children,
    )
}
