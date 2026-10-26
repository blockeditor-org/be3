use crate::frame::{ColorRole, Frame};
use beui_macros::{component, view};

use crate::tokens::{Length, Size, Stroke};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;

fn measurement_along(
    axis: Direction,
    direction: Prop<Direction>,
    length: Prop<Length>,
) -> Memo<Length> {
    create_memo(move || match direction.get() == axis {
        true => length.get(),
        false => Length::Size(Size::Custom(Stroke::Hairline.px())),
    })
}

#[component]
pub fn Separator(
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
    #[prop(default = Length::INTRINSIC)] length: Prop<Length>,
) -> NodeId {
    let width = measurement_along(Direction::Horizontal, direction, length);
    let height = measurement_along(Direction::Vertical, direction, length);
    view! {
        <Frame fill=ColorRole::Border width=width height=height />
    }
}
