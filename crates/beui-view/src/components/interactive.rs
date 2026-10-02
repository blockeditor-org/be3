use crate::reactive::{Callback, Child, ClickCallback, Prop, create_effect, with_document};
use beui_core::base::list::Direction;
use beui_core::geometry::{Pos2, Vec2};
use beui_core::input::{
    AutoscrollGesture, CursorIcon, DragGesture, ImeEvent, ImeText, KeyPress, PointerPress,
    ScrollGesture, SecondaryDrag, ZoomGesture,
};
use beui_core::node::NodeId;

use beui_core::base::focus::ImeCursor;
use beui_core::base::interactive::InteractiveNode;
use beui_core::interact::forward::ForwardedInput;
use beui_macros::component;

#[component]
pub fn Interactive(
    #[prop(default = false)] focusable: bool,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] ime: Prop<bool>,
    #[prop(default = None)] ime_cursor: Prop<Option<ImeCursor>>,
    #[prop(default = None)] ime_text: Prop<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    #[prop(default = true)] takes_text: bool,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_ancestor_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
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
    intercept_at: Callback<Pos2, bool>,
    on_forward: Callback<ForwardedInput>,
    forward_at: Callback<Pos2, bool>,
    children: Option<Child>,
) -> NodeId {
    assert!(
        focusable
            || (on_focus_change.is_empty()
                && on_activate_change.is_empty()
                && on_activate.is_empty()
                && on_step.is_empty()
                && on_text.is_empty()
                && on_ime.is_empty()
                && on_key.is_empty()
                && on_ancestor_key.is_empty()
                && on_motion.is_empty()),
        "an Interactive handed focus callbacks has to be focusable"
    );
    let interactive = with_document(|document| {
        let interactive = document.create_interactive(focusable);
        let node = document.arena.touch_mut_as::<InteractiveNode>(interactive);
        if let Some(focus) = node.focus.as_mut() {
            focus.on_focus_change = on_focus_change;
            focus.on_activate_change = on_activate_change;
            focus.on_activate = on_activate;
            focus.on_step = on_step;
            if takes_text {
                focus.on_text = on_text;
            }
            focus.on_ime = on_ime;
            focus.on_key = on_key;
            focus.on_ancestor_key = on_ancestor_key;
            focus.on_motion = on_motion;
        }
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
        node.intercept_at = intercept_at;
        node.on_forward = on_forward;
        node.forward_at = forward_at;
        if let Some(child) = children {
            document.set_interactive_child(interactive, child);
        }
        interactive
    });
    if focusable {
        create_effect(move || {
            with_document(|document| document.set_focusable_tab_stop(interactive, tab_stop.get()))
        });
        create_effect(move || {
            with_document(|document| {
                document.set_focusable_press_focus(interactive, press_focus.get())
            })
        });
        create_effect(move || {
            with_document(|document| document.set_focusable_ime(interactive, ime.get()))
        });
        create_effect(move || {
            let cursor = ime_cursor.get();
            with_document(|document| document.set_focusable_ime_cursor(interactive, cursor))
        });
        create_effect(move || {
            let text = ime_text.get();
            with_document(|document| document.set_focusable_ime_text(interactive, text))
        });
        create_effect(move || {
            let wanted = focused.get();
            with_document(|document| match wanted {
                true => document.focus_focusable(interactive.id()),
                false if document.focused_node() == Some(interactive.id()) => {
                    document.update_focus(None)
                }
                false => {}
            });
        });
    }
    if let Some(cursor) = cursor {
        create_effect(move || {
            with_document(|document| {
                document.set_interactive_cursor(interactive, Some(cursor.get()))
            })
        });
    }
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_capture_presses(interactive, capture_presses.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_repeat_drag(interactive, repeat_drag.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_touch_drags(interactive, touch_drags.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_touch_drag_axis(interactive, touch_drag_axis.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_claims_touch(interactive, claims_touch.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_interactive_scroll_axis(interactive, scroll_axis.get())
        })
    });
    create_effect(move || {
        with_document(|document| document.set_interactive_key_active(interactive, key_active.get()))
    });
    interactive.id()
}
