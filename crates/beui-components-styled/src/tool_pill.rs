use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::list::List;
use crate::scroll::Scroll;
use crate::tokens::Space;
use beui_view::types::{Align, Direction, ItemSize};

#[component]
pub fn ToolPill(
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
    children: Children<ListChild>,
) -> NodeId {
    let row = NodeRef::new();
    arrow_focus(row, direction);
    view! {
        <Frame fill=ColorRole::SurfaceRaised border={Border::DEFAULT} radius=Radius::Full>
            <List direction={direction} spacing=Space::None>
                <Scroll
                    @sizing={ItemSize::Intrinsic.shrink(1.0)}
                    direction={direction}
                    fit=true
                >
                    <Frame padding=Space::Xs>
                        <List
                            @node_ref=&row
                            direction={direction}
                            align=Align::Center
                            spacing=Space::Xs2
                            children
                        />
                    </Frame>
                </Scroll>
            </List>
        </Frame>
    }
}
