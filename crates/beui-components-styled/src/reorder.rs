use std::hash::Hash;

use beui_macros::{component, view};
use beui_view::primitives::Layers;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{CursorIcon, Direction, Vec2};

use beui_components_unstyled::{ReorderHandle, ReorderItemHandle, ReorderMove};
use beui_view::drag::{DragPoint, Take};

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::tokens::{Movement, Padding, Space, Stroke};

pub const LIFT_DROP: Space = Space::Xs;

/// `unstyled::Reorder` settling its rows and gaps on the theme's `Movement::Settle`.
#[component]
pub fn Reorder<K, P>(
    value: Prop<Vec<K>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    payload: Func<K, Option<P>>,
    #[prop(default = None)] accepts: Option<Func<P, bool>>,
    #[prop(default = None)] takes: Option<Func<DragPoint, Take>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] fixed: Prop<bool>,
    #[prop(default = None)] slot: Option<Func<P, Option<usize>>>,
    #[prop(default = false)] long_press: Prop<bool>,
    text: Callback<K, String>,
    on_move: Callback<ReorderMove<K, P>>,
    on_drag_change: Callback<Option<K>>,
    on_drop_outside: Callback<(K, DragPoint)>,
    preview: RenderFn<P, NodeId>,
    #[prop(children)] content: Render<ReorderHandle<K, P>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    view! {
        <beui_components_unstyled::Reorder
            value
            spring={Movement::Settle.spring()}
            direction
            payload
            accepts
            takes
            disabled
            fixed
            slot
            long_press
            text={text}
            on_move={on_move}
            on_drag_change={on_drag_change}
            on_drop_outside={on_drop_outside}
            preview
        >|handle: ReorderHandle<K, P>| {
            content.call(handle)
        }</beui_components_unstyled::Reorder>
    }
}

#[component]
pub fn ReorderItem<K, P>(
    reorder: ReorderHandle<K, P>,
    item: Prop<K>,
    #[prop(default = Radius::Md)] radius: Prop<Radius>,
    #[prop(default = Padding::ZERO)] margin: Prop<Padding>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    on_click: Callback<()>,
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
                true => ColorRole::Control,
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
