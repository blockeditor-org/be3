use beui_macros::{component, view};

use crate::theme::{BORDER_WIDTH, SEPARATOR_THICKNESS, use_theme};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn Bordered(corner_radius: f32, children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={create_memo(move || Some(Outline::new(theme.border.get(), BORDER_WIDTH)))}
            radius=corner_radius
        >
            {children}
        </Frame>
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
    let theme = use_theme();
    let width = measurement_along(Direction::Horizontal, direction, length);
    let height = measurement_along(Direction::Vertical, direction, length);
    view! {
        <Frame fill={theme.border} width=width height=height radius=0.0 />
    }
}
