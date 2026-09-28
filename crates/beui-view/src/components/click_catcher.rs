use crate::reactive::{Callback, Child, ClickCallback, Prop, create_effect, with_document};
use beui_core::base::list::Direction;
use beui_core::geometry::{Pos2, Vec2};
use beui_core::input::{
    AutoscrollGesture, CursorIcon, DragGesture, PointerPress, ScrollGesture, SecondaryDrag,
    ZoomGesture,
};
use beui_core::node::NodeId;

use beui_core::base::click_catcher::ClickCatcherNode;
use beui_macros::component;

#[component]
pub fn ClickCatcher(
    cursor: Option<Prop<CursorIcon>>,
    #[prop(default = false)] key_active: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = false)] repeat_drag: Prop<bool>,
    #[prop(default = false)] touch_drags: Prop<bool>,
    #[prop(default = None)] touch_drag_axis: Prop<Option<Direction>>,
    #[prop(default = true)] claims_touch: Prop<bool>,
    #[prop(default = None)] scroll_axis: Prop<Option<Direction>>,
    on_click: ClickCallback,
    on_click_at: Callback<PointerPress>,
    on_hover_change: Callback<bool>,
    on_hover_move: Callback<PointerPress>,
    on_active_change: Callback<bool>,
    on_cancel: ClickCallback,
    on_middle_click: ClickCallback,
    on_press: Callback<PointerPress>,
    on_secondary_press: Callback<PointerPress>,
    on_secondary_drag: Callback<SecondaryDrag>,
    on_drag: Callback<PointerPress>,
    on_pan_drag: Callback<Vec2>,
    on_pan_active_change: Callback<bool>,
    on_scroll: Callback<ScrollGesture>,
    on_scroll_drag: Callback<DragGesture>,
    on_autoscroll: Callback<AutoscrollGesture>,
    on_zoom: Callback<ZoomGesture>,
    capture_at: Callback<Pos2, bool>,
    children: Option<Child>,
) -> NodeId {
    let click_catcher = with_document(|document| {
        let click_catcher = document.create_click_catcher();
        let node = document
            .arena
            .touch_mut_as::<ClickCatcherNode>(click_catcher);
        node.on_click = on_click;
        node.on_click_at = on_click_at;
        node.on_hover_change = on_hover_change;
        node.on_hover_move = on_hover_move;
        node.on_active_change = on_active_change;
        node.on_cancel = on_cancel;
        node.on_middle_click = on_middle_click;
        node.on_press = on_press;
        node.on_secondary_press = on_secondary_press;
        node.on_secondary_drag = on_secondary_drag;
        node.on_drag = on_drag;
        node.on_pan_drag = on_pan_drag;
        node.on_pan_active_change = on_pan_active_change;
        node.on_scroll = on_scroll;
        node.on_scroll_drag = on_scroll_drag;
        node.on_autoscroll = on_autoscroll;
        node.on_zoom = on_zoom;
        node.capture_at = capture_at;
        if let Some(child) = children {
            document.set_click_catcher_child(click_catcher, child);
        }
        click_catcher
    });
    if let Some(cursor) = cursor {
        create_effect(move || {
            with_document(|document| {
                document.set_click_catcher_cursor(click_catcher, Some(cursor.get()))
            })
        });
    }
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_capture_presses(click_catcher, capture_presses.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_repeat_drag(click_catcher, repeat_drag.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_touch_drags(click_catcher, touch_drags.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_touch_drag_axis(click_catcher, touch_drag_axis.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_claims_touch(click_catcher, claims_touch.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_scroll_axis(click_catcher, scroll_axis.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_key_active(click_catcher, key_active.get())
        })
    });
    click_catcher
}
