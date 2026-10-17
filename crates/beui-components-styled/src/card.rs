use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use beui_core::node::NodeId;
use beui_view::reactive::*;

pub(crate) const PADDING_HORIZONTAL: f32 = 18.0;
pub(crate) const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn Card(children: Child) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            {children}
        </Frame>
    }
}
