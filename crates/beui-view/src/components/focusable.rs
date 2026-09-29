use crate::reactive::{Callback, Child, ClickCallback, Prop, create_effect, with_document};
use beui_core::geometry::Vec2;
use beui_core::input::KeyPress;
use beui_core::node::NodeId;

use beui_core::base::focusable::{FocusableNode, ImeCursor};
use beui_macros::component;

#[component]
pub fn Focusable(
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] ime: Prop<bool>,
    #[prop(default = None)] ime_cursor: Prop<Option<ImeCursor>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    on_preedit: Callback<String>,
    on_key: Callback<KeyPress, bool>,
    on_ancestor_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    children: Option<Child>,
) -> NodeId {
    let focusable = with_document(|document| {
        let focusable = document.create_focusable();
        let node = document.arena.touch_mut_as::<FocusableNode>(focusable);
        node.on_focus_change = on_focus_change;
        node.on_activate_change = on_activate_change;
        node.on_activate = on_activate;
        node.on_step = on_step;
        node.on_text = on_text;
        node.on_preedit = on_preedit;
        node.on_key = on_key;
        node.on_ancestor_key = on_ancestor_key;
        node.on_motion = on_motion;
        if let Some(child) = children {
            document.set_focusable_child(focusable, child);
        }
        focusable
    });
    create_effect(move || {
        with_document(|document| document.set_focusable_tab_stop(focusable, tab_stop.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_focusable_press_focus(focusable, press_focus.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_focusable_ime(focusable, ime.get()))
    });
    create_effect(move || {
        let cursor = ime_cursor.get();
        with_document(|document| document.set_focusable_ime_cursor(focusable, cursor))
    });
    create_effect(move || {
        let wanted = focused.get();
        with_document(|document| match wanted {
            true => document.focus_focusable(focusable),
            false if document.focused_node() == Some(focusable) => document.update_focus(None),
            false => {}
        });
    });
    focusable
}
