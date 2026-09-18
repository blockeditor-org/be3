use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Direction, Frame, ItemSize, List, Prop, create_memo};
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

#[component]
pub fn Separator(#[prop(default = Direction::Horizontal)] direction: Prop<Direction>) -> NodeId {
    let theme = use_theme();
    let across = create_memo(move || cross_axis(direction.get()));
    view! {
        <List direction={across} spacing=0.0>
            <Frame
                @sizing=ItemSize::Fixed(SEPARATOR_THICKNESS)
                color={theme.border.clone()}
                radius=0
            />
        </List>
    }
}

fn cross_axis(direction: Direction) -> Direction {
    match direction {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    }
}
