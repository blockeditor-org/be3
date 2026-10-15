use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::theme::SEPARATOR_THICKNESS;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Bordered(corner_radius: Radius, children: Child) -> NodeId {
    view! {
        <Frame border={Border::DEFAULT} radius=corner_radius>{children}</Frame>
    }
}

fn measurement_along(
    axis: Direction,
    direction: Prop<Direction>,
    length: Prop<Option<f32>>,
) -> Memo<Option<f32>> {
    create_memo(move || match direction.get() == axis {
        true => length.get(),
        false => Some(SEPARATOR_THICKNESS),
    })
}

#[component]
pub fn Separator(
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
    #[prop(default = None)] length: Prop<Option<f32>>,
) -> NodeId {
    let width = measurement_along(Direction::Horizontal, direction, length);
    let height = measurement_along(Direction::Vertical, direction, length);
    view! {
        <Frame fill=ColorRole::Border width=width height=height />
    }
}
