use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::Rect;
use beui_view::types::{Image, ImageFit, Thumbhash};

use crate::frame::Radius;
use beui_components_unstyled as unstyled;

#[component]
pub fn Picture(
    image: Prop<Option<Image>>,
    #[prop(default = None)] thumbhash: Prop<Option<Thumbhash>>,
    #[prop(default = None)] source: Prop<Option<Rect>>,
    #[prop(default = ImageFit::Contain)] fit: Prop<ImageFit>,
    #[prop(default = Color32::WHITE)] tint: Prop<Color32>,
    #[prop(default = Radius::None)] radius: Prop<Radius>,
    #[prop(default = true)] smooth: Prop<bool>,
) -> NodeId {
    let radius = radius.map(Radius::px);
    view! {
        <unstyled::Picture image thumbhash source fit tint radius smooth />
    }
}
