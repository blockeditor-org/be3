use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::list::List;
use crate::scroll::Scroll;
use crate::tokens::Space;
use beui_view::types::{Align, Direction, ItemSize};

/// The fill of a toolbar band, which beui-dock-host's band is drawn with too.
pub const BAND_FILL: ColorRole = ColorRole::Surface;

/// The raised, rounded bar a floating set of tools sits in, or with `band` a flat
/// toolbar band. Arrow keys move focus along it. Offered less than it needs, it
/// lets `.shrink(..)` children narrow (through `least_length`) before it scrolls.
/// Placing it is the caller's.
#[component]
pub fn ToolPill(
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
    #[prop(default = false)] band: Prop<bool>,
    #[prop(default = Space::Xs2)] spacing: Prop<Space>,
    children: Children<ListChild>,
) -> NodeId {
    let row = NodeRef::new();
    arrow_focus(row, direction);
    let fill = create_memo(move || match band.get() {
        true => BAND_FILL,
        false => ColorRole::Control,
    });
    let border = create_memo(move || (!band.get()).then_some(Border::DEFAULT));
    let radius = create_memo(move || match band.get() {
        true => Radius::None,
        false => Radius::Full,
    });
    view! {
        <Frame fill={fill} border={border} radius={radius}>
            <List direction={direction} spacing=Space::None>
                <Scroll @sizing={ItemSize::Intrinsic.shrink(1.0)} direction={direction} fit=true>
                    <Frame @sizing={ItemSize::Intrinsic.shrink(1.0)} padding=Space::Xs>
                        <List
                            @node_ref=&row
                            direction={direction}
                            align=Align::Center
                            spacing={spacing}
                            children
                        />
                    </Frame>
                </Scroll>
            </List>
        </Frame>
    }
}
