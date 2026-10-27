use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::Get;
use crate::reactive::ImeCursor;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::CursorIcon;
use crate::types::PasteKind;
use crate::types::Pasted;
use crate::types::Pos2;
use crate::types::SoftKeyboard;
use crate::types::TouchDrags;
use crate::types::Vec2;
use crate::types::input::ImeEvent;
use crate::types::input::ImeText;
use crate::types::input::KeyPress;
use crate::types::input::PointerInput;
use crate::types::input::PointerPress;
use crate::types::input::WheelInput;
use crate::types::region::{Area, Claim};
use beui_core_html::document::{create, set_style, wrap};
use beui_core_html::interactive::{ForwardedInput, Handlers, Switches, TabStop};
use std::rc::Rc;

use super::bind;

pub(crate) fn interactive(
    focusable: bool,
    tab_stop: Prop<bool>,
    yields_tab_stop: bool,
    press_focus: Prop<bool>,
    focused: Prop<bool>,
    ime: Prop<bool>,
    keyboard: Prop<SoftKeyboard>,
    ime_cursor: Prop<Option<ImeCursor>>,
    ime_text: Prop<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    pastes: Prop<Vec<PasteKind>>,
    on_paste: Callback<Pasted>,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    cursor: Prop<Option<CursorIcon>>,
    pointer_locked: Prop<bool>,
    key_active: Prop<bool>,
    capture_presses: Prop<Area>,
    repeat_drag: Prop<bool>,
    touch_drags: Prop<TouchDrags>,
    claims_touch: Prop<bool>,
    drawing: Prop<Area>,
    every_sample: Prop<bool>,
    grabs_pointer: Prop<bool>,
    on_click: ClickCallback,
    on_click_at: Callback<PointerPress>,
    on_hover_change: Callback<bool>,
    on_hover_move: Callback<PointerPress>,
    on_active_change: Callback<bool>,
    on_cancel: ClickCallback,
    on_middle_click: ClickCallback,
    on_press: Callback<PointerPress>,
    on_secondary_press: Callback<PointerPress>,
    on_long_press_drag: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    on_pointer: Callback<PointerInput, bool>,
    on_wheel: Callback<WheelInput, bool>,
    gesture_area: Prop<Area>,
    hit_area: Prop<Area>,
    on_forward: Callback<ForwardedInput>,
    inhibits_shortcuts: Prop<bool>,
    clipboard_keys: Prop<bool>,
    forwards_wheel: Prop<bool>,
    claims: Prop<Vec<Claim>>,
    on_press_outside: Callback<Pos2>,
    children: Option<Child>,
) -> NodeId {
    let _ = (
        pastes,
        key_active,
        claims_touch,
        inhibits_shortcuts,
        clipboard_keys,
        forwards_wheel,
    );
    let node = match children {
        Some(child) => wrap("Interactive", child),
        None => create("Interactive", "div"),
    };
    set_style(node, "pointer-events", Some("auto".to_owned()));
    if focusable {
        let stop = TabStop::new(node, yields_tab_stop);
        bind(tab_stop, move |tab_stop| stop.set(tab_stop));
        bind(focused, move |focused| {
            if focused {
                beui_core_html::focus::set_focus(Some(node));
            }
        });
    }
    bind(cursor, move |cursor| {
        set_style(
            node,
            "cursor",
            cursor.map(|cursor| cursor.name().to_owned()),
        );
    });
    let switches = Rc::new(Switches::default());
    let held = switches.clone();
    bind(press_focus, move |press_focus| {
        held.press_focus.set(press_focus)
    });
    let held = switches.clone();
    bind(ime, move |ime| held.ime.set(ime));
    let held = switches.clone();
    bind(hit_area, move |area| *held.hit_area.borrow_mut() = area);
    let held = switches.clone();
    bind(capture_presses, move |area| {
        *held.capture_presses.borrow_mut() = area
    });
    let held = switches.clone();
    bind(claims, move |claims| *held.claims.borrow_mut() = claims);
    let held = switches.clone();
    bind(every_sample, move |every| held.every_sample.set(every));
    let held = switches.clone();
    bind(grabs_pointer, move |grabs| held.grabs_pointer.set(grabs));
    let held = switches.clone();
    bind(repeat_drag, move |repeat| held.repeat_drag.set(repeat));
    let held = switches.clone();
    bind(pointer_locked, move |locked| {
        held.pointer_locked.set(locked);
        beui_core_html::interactive::lock_pointer(node, locked);
    });
    let held = switches.clone();
    bind(keyboard, move |keyboard| {
        held.keyboard.set(keyboard);
        beui_core_html::ime::keyboard(node, keyboard);
    });
    create_binding(move || {
        let action = touch_action(touch_drags.get(), &gesture_area.get(), &drawing.get());
        set_style(node, "touch-action", action.map(str::to_owned));
    });
    create_binding(move || beui_core_html::ime::sync(ime_text.get().as_ref()));
    create_binding(move || {
        beui_core_html::ime::place(ime_cursor.get().and_then(|cursor| cursor.rect));
    });
    beui_core_html::interactive::decorate(
        node,
        Handlers {
            on_focus_change,
            on_activate,
            on_key,
            on_text,
            on_ime,
            on_paste,
            on_click,
            on_click_at,
            on_hover_change,
            on_hover_move,
            on_active_change,
            on_cancel,
            on_middle_click,
            on_press,
            on_secondary_press,
            on_long_press_drag,
            on_drag,
            on_pointer,
            on_forward,
            on_wheel,
            on_activate_change,
            on_step,
            on_motion,
            on_press_outside,
        },
        switches,
    );
    node
}

fn touch_action(drags: TouchDrags, gestures: &Area, drawing: &Area) -> Option<&'static str> {
    if *gestures != Area::Nowhere || *drawing != Area::Nowhere {
        return Some("none");
    }
    match drags {
        TouchDrags::None => None,
        TouchDrags::All => Some("none"),
        TouchDrags::Along(crate::types::Direction::Horizontal) => Some("pan-y"),
        TouchDrags::Along(crate::types::Direction::Vertical) => Some("pan-x"),
    }
}
