use crate::reactive::Children;
use crate::reactive::ListChild;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Align;
use crate::types::Direction;
use crate::types::Justify;
use beui_macros::component;

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<ListChild>,
) -> NodeId {
    crate::core::list(direction, align, justify, wrap, spacing, children)
}

#[component]
pub fn Spacer() -> NodeId {
    crate::core::spacer()
}
