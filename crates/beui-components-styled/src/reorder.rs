use std::hash::Hash;

use beui_macros::{component, view};
use beui_view::primitives::Layers;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{CursorIcon, Vec2};

use beui_components_unstyled::{ReorderHandle, ReorderItemHandle};

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::tokens::{Padding, Space, Stroke};

pub const LIFT_DROP: Space = Space::Xs;

#[component]
pub fn ReorderItem<K, P>(
    reorder: ReorderHandle<K, P>,
    item: Prop<K>,
    #[prop(default = Radius::Md)] radius: Prop<Radius>,
    #[prop(default = Padding::ZERO)] margin: Prop<Padding>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    on_click: ClickCallback,
    #[prop(children)] content: Render<ReorderItemHandle>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    view! {
        <beui_components_unstyled::ReorderItem
            reorder
            item
            capture_presses
            cursor
            on_click
        >|handle: ReorderItemHandle| {
            let lifted = handle.lifted;
            let face = content.call(handle);
            let shadow = create_memo(move || match lifted.get() {
                true => ColorRole::ScrimSoft,
                false => ColorRole::None,
            });
            let fill = create_memo(move || match lifted.get() {
                true => ColorRole::SurfaceRaised,
                false => ColorRole::None,
            });
            let border = create_memo(move || {
                lifted
                    .get()
                    .then(|| Border::new(ColorRole::Border).width(Stroke::Hairline))
            });
            let drop = create_memo(move || match lifted.get() {
                true => Vec2::new(0.0, LIFT_DROP.px()),
                false => Vec2::ZERO,
            });
            view! {
                <Frame padding={margin}>
                    <Layers>
                        <Frame fill={shadow} radius translate={drop} />
                        <Frame fill={fill} border={border} radius>{face}</Frame>
                    </Layers>
                </Frame>
            }
        }</beui_components_unstyled::ReorderItem>
    }
}
