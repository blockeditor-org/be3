use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::reactive::create_effect;
use crate::reactive::on_cleanup;
use crate::reactive::with_document;
use beui_core::focus::ImeCursor;
use beui_core::base::interactive::InteractiveNode;
use beui_core::base::list::Direction;
use beui_core::clipboard::PasteKind;
use beui_core::clipboard::Pasted;
use beui_core::geometry::Pos2;
use beui_core::geometry::Vec2;
use beui_core::input::AutoscrollGesture;
use beui_core::input::CursorIcon;
use beui_core::input::DragGesture;
use beui_core::input::ImeEvent;
use beui_core::input::ImeText;
use beui_core::input::KeyPress;
use beui_core::input::Modifiers;
use beui_core::input::PointerPress;
use beui_core::input::ScrollGesture;
use beui_core::input::SecondaryDrag;
use beui_core::input::ZoomGesture;
use beui_core::interact::forward::ForwardedInput;
use beui_core::node::NodeId;
use std::cell::Cell;
use std::rc::Rc;

pub fn interactive(
    focusable: bool,
    tab_stop: Prop<bool>,
    yields_tab_stop: bool,
    press_focus: Prop<bool>,
    focused: Prop<bool>,
    ime: Prop<bool>,
    keyboard_on_focus: bool,
    keyboard_asks: Prop<Option<u32>>,
    ime_keyboard: Prop<bool>,
    ime_cursor: Prop<Option<ImeCursor>>,
    ime_text: Prop<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    takes_text: bool,
    pastes: Prop<Vec<PasteKind>>,
    on_paste: Callback<Pasted>,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    cursor: Option<Prop<CursorIcon>>,
    pointer_locked: Prop<bool>,
    key_active: Prop<bool>,
    capture_presses: Prop<bool>,
    repeat_drag: Prop<bool>,
    touch_drags: Prop<bool>,
    touch_drag_axis: Prop<Option<Direction>>,
    claims_touch: Prop<bool>,
    drawing: Prop<bool>,
    every_sample: Prop<bool>,
    scroll_axis: Prop<Option<Direction>>,
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
    gestures_at: Callback<Pos2, bool>,
    hit_at: Callback<Pos2, bool>,
    drawing_at: Callback<Pos2, bool>,
    on_forward: Callback<ForwardedInput>,
    inhibits_shortcuts: Prop<bool>,
    clipboard_keys: Prop<bool>,
    forwards_wheel: Prop<bool>,
    claim_modifiers: Prop<Option<Modifiers>>,
    claim_at: Callback<(Pos2, Modifiers), bool>,
    on_press_outside: Callback<Pos2>,
    children: Option<Child>,
) -> NodeId {
    assert!(
        focusable
            || (on_focus_change.is_empty()
                && on_activate_change.is_empty()
                && on_activate.is_empty()
                && on_step.is_empty()
                && on_text.is_empty()
                && on_paste.is_empty()
                && on_ime.is_empty()
                && on_key.is_empty()
                && on_motion.is_empty()),
        "an Interactive handed focus callbacks has to be focusable"
    );
    let on_focus_change = match on_focus_change.is_empty() {
        true => on_focus_change,
        false => {
            let held = Rc::new(Cell::new(false));
            let holding = Rc::clone(&held);
            on_cleanup(move || {
                let live = beui_core::current::try_with_document(|_| ()).is_some();
                if held.get() && live {
                    on_focus_change.call(false);
                }
            });
            Callback::new(move |focused: bool| {
                holding.set(focused);
                on_focus_change.call(focused);
            })
        }
    };
    let interactive = with_document(|document| {
        let interactive = document.create_interactive(focusable);
        let node = document.arena.touch_mut_as::<InteractiveNode>(interactive);
        if let Some(focus) = node.focus.as_mut() {
            focus.keyboard_on_focus = keyboard_on_focus;
            focus.yields_tab_stop = yields_tab_stop;
            focus.on_focus_change = on_focus_change;
            focus.on_activate_change = on_activate_change;
            focus.on_activate = on_activate;
            focus.on_step = on_step;
            if takes_text {
                focus.on_text = on_text;
            }
            focus.on_paste = on_paste;
            focus.on_ime = on_ime;
            focus.on_key = on_key;
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
        node.gestures_at = gestures_at;
        node.hit_at = hit_at;
        node.drawing_at = drawing_at;
        node.on_forward = on_forward;
        document.set_interactive_claim_at(interactive, claim_at);
        document.set_interactive_on_press_outside(interactive, on_press_outside);
        if let Some(child) = children {
            document.set_interactive_child(interactive, child);
        }
        interactive
    });
    if focusable {
        create_binding(move || {
            let (tab_stop, ime) = (tab_stop.get(), ime.get());
            let (pastes, keyboard, asks) = (pastes.get(), ime_keyboard.get(), keyboard_asks.get());
            with_document(|document| {
                document.set_focusable_tab_stop(interactive, tab_stop);
                document.set_focusable_ime(interactive, ime);
                document.set_focusable_pastes(interactive, pastes);
                document.set_focusable_ime_keyboard(interactive, keyboard);
                document.set_focusable_keyboard_asks(interactive, asks);
            })
        });
        create_binding(move || {
            let cursor = ime_cursor.get();
            with_document(|document| document.set_focusable_ime_cursor(interactive, cursor))
        });
        create_binding(move || {
            let text = ime_text.get();
            with_document(|document| document.set_focusable_ime_text(interactive, text))
        });
        let asked = Cell::new(false);
        create_effect(move || {
            let wanted = focused.get();
            if !wanted && !asked.replace(wanted) {
                return;
            }
            with_document(|document| match wanted {
                true => document.focus_focusable(interactive.id()),
                false if document.focused_node() == Some(interactive.id()) => {
                    document.update_focus(None)
                }
                false => {}
            });
        });
    }
    create_binding(move || {
        let locked = pointer_locked.get();
        with_document(|document| document.set_interactive_locks_pointer(interactive, locked));
    });
    if let Some(cursor) = cursor {
        create_binding(move || {
            let cursor = cursor.get();
            with_document(|document| document.set_interactive_cursor(interactive, Some(cursor)))
        });
    }
    create_binding(move || {
        let (capture, repeat, touch) =
            (capture_presses.get(), repeat_drag.get(), touch_drags.get());
        let pressing = press_focus.get();
        let (axis, claims, scroll) = (touch_drag_axis.get(), claims_touch.get(), scroll_axis.get());
        let (draws, every) = (drawing.get(), every_sample.get());
        let (active, inhibits, claimed) = (
            key_active.get(),
            inhibits_shortcuts.get(),
            claim_modifiers.get(),
        );
        let wheel = forwards_wheel.get();
        let clipboard = clipboard_keys.get();
        with_document(|document| {
            document.set_interactive_clipboard_keys(interactive, clipboard);
            document.set_interactive_capture_presses(interactive, capture);
            document.set_interactive_press_focus(interactive, pressing);
            document.set_interactive_repeat_drag(interactive, repeat);
            document.set_interactive_touch_drags(interactive, touch);
            document.set_interactive_touch_drag_axis(interactive, axis);
            document.set_interactive_claims_touch(interactive, claims);
            document.set_interactive_drawing(interactive, draws);
            document.set_interactive_every_sample(interactive, every);
            document.set_interactive_scroll_axis(interactive, scroll);
            document.set_interactive_key_active(interactive, active);
            document.set_interactive_inhibits_shortcuts(interactive, inhibits);
            document.set_interactive_claim_modifiers(interactive, claimed);
            document.set_interactive_forwards_wheel(interactive, wheel);
        })
    });
    interactive.id()
}
