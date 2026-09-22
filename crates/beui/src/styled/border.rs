use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Direction, Frame, Memo, Prop, create_memo};
use crate::styled::theme::{BORDER_WIDTH, SEPARATOR_THICKNESS, use_theme};

#[component]
pub fn Bordered(corner_radius: u8, children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            radius={corner_radius}
            outline_offset=0.0
            outline_visible=true
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
    let width = measurement_along(Direction::Horizontal, direction.clone(), length.clone());
    let height = measurement_along(Direction::Vertical, direction, length);
    view! {
        <Frame color={theme.border.clone()} width={width} height={height} radius=0 />
    }
}
