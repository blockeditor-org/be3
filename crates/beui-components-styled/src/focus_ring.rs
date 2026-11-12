use beui_macros::{component, view};

use crate::frame::{Frame, Radius};
use crate::tokens::Stroke;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

/// A frame around `children` that draws the keyboard focus ring `offset` outside them, its
/// corners following `radius` grown by that offset: a `Frame` with `focus`.
#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = Radius::Md)] radius: Radius,
    #[prop(default = Stroke::None)] offset: Stroke,
    children: Child,
) -> NodeId {
    let around = Radius::Custom(radius.px() + offset.px());
    view! {
        <Frame focus={focused} focus_offset={offset} radius={around}>{children}</Frame>
    }
}
