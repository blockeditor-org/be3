use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use beui_core::node::NodeId;
use beui_view::reactive::*;

use crate::tokens::Space;

pub(crate) const PADDING: Space = Space::Xl;

#[component]
pub fn Card(children: Child) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
            padding=PADDING
        >
            {children}
        </Frame>
    }
}
