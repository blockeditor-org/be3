use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Func;
use crate::reactive::ImeCursor;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::CursorIcon;
use crate::types::CursorImage;
use crate::types::ImeEvent;
use crate::types::ImeText;
use crate::types::KeyPress;

use crate::types::ClipItem;
use crate::types::PointerPress;
use crate::types::Pos2;
use crate::types::SoftKeyboard;
use crate::types::TouchDrags;
use crate::types::Vec2;
use crate::types::forward::ForwardedInput;
use crate::types::input::PointerInput;
use crate::types::input::WheelInput;
use crate::types::region::{Area, Claim};
use beui_macros::component;

#[component]
pub fn Interactive(
    #[prop(default = false)] focusable: bool,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = false)] yields_tab_stop: bool,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] ime: Prop<bool>,
    #[prop(default = SoftKeyboard::OnFocus)] keyboard: Prop<SoftKeyboard>,
    #[prop(default = None)] ime_cursor: Prop<Option<ImeCursor>>,
    #[prop(default = None)] ime_text: Prop<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: Callback<()>,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    #[prop(default = Vec::new())] pastes: Prop<Vec<String>>,
    on_paste: Callback<ClipItem>,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    #[prop(default = None)] cursor: Prop<Option<CursorIcon>>,
    #[prop(default = None)] cursor_image: Prop<Option<CursorImage>>,
    #[prop(default = false)] pointer_locked: Prop<bool>,
    #[prop(default = false)] key_active: Prop<bool>,
    #[prop(default = Area::Nowhere)] capture_presses: Prop<Area>,
    #[prop(default = false)] repeat_drag: Prop<bool>,
    #[prop(default = TouchDrags::None)] touch_drags: Prop<TouchDrags>,
    #[prop(default = true)] claims_touch: Prop<bool>,
    #[prop(default = Area::Nowhere)] drawing: Prop<Area>,
    #[prop(default = false)] every_sample: Prop<bool>,
    #[prop(default = false)] grabs_pointer: Prop<bool>,
    on_click: Callback<()>,
    on_click_at: Callback<PointerPress>,
    on_hover_change: Callback<bool>,
    on_hover_move: Callback<PointerPress>,
    on_pointed_change: Callback<bool>,
    on_active_change: Callback<bool>,
    on_cancel: Callback<()>,
    on_middle_click: Callback<()>,
    on_press: Callback<PointerPress>,
    on_secondary_press: Callback<PointerPress>,
    on_long_press_drag: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    #[prop(default = None)] drag: Option<Func<PointerPress, Option<crate::drag::Carry>>>,
    on_pointer: Callback<PointerInput, bool>,
    on_wheel: Callback<WheelInput, bool>,
    #[prop(default = Area::Nowhere)] gesture_area: Prop<Area>,
    #[prop(default = Area::Whole)] hit_area: Prop<Area>,
    on_forward: Callback<ForwardedInput>,
    #[prop(default = false)] inhibits_shortcuts: Prop<bool>,
    #[prop(default = false)] clipboard_keys: Prop<bool>,
    #[prop(default = true)] forwards_wheel: Prop<bool>,
    #[prop(default = Vec::new())] claims: Prop<Vec<Claim>>,
    on_press_outside: Callback<Pos2>,
    children: Option<Child>,
) -> NodeId {
    let (on_press, on_drag) = match drag {
        Some(drag) => lifting(drag, on_press, on_drag),
        None => (on_press, on_drag),
    };
    crate::core::interactive(
        focusable,
        tab_stop,
        yields_tab_stop,
        press_focus,
        focused,
        ime,
        keyboard,
        ime_cursor,
        ime_text,
        on_focus_change,
        on_activate_change,
        on_activate,
        on_step,
        on_text,
        pastes,
        on_paste,
        on_ime,
        on_key,
        on_motion,
        cursor,
        cursor_image,
        pointer_locked,
        key_active,
        capture_presses,
        repeat_drag,
        touch_drags,
        claims_touch,
        drawing,
        every_sample,
        grabs_pointer,
        on_click,
        on_click_at,
        on_hover_change,
        on_hover_move,
        on_pointed_change,
        on_active_change,
        on_cancel,
        on_middle_click,
        on_press,
        on_secondary_press,
        on_long_press_drag,
        on_drag,
        on_pointer,
        on_wheel,
        gesture_area,
        hit_area,
        on_forward,
        inhibits_shortcuts,
        clipboard_keys,
        forwards_wheel,
        claims,
        on_press_outside,
        children,
    )
}

/// Wraps a press's callbacks so the first drag of each press asks `drag` for a carry and
/// lifts it onto the tree's drag driver; once lifted, the driver follows the pointer.
fn lifting(
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    on_press: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
) -> (Callback<PointerPress>, Callback<PointerPress>) {
    let asked = std::rc::Rc::new(std::cell::Cell::new(false));
    // Whether this press lifted a carry: the board follows its drags from then on, so they
    // are not this node's to hear.
    let lifted = std::rc::Rc::new(std::cell::Cell::new(false));
    let origin = std::rc::Rc::new(std::cell::Cell::new(None::<PointerPress>));
    let pressed = {
        let (asked, lifted, origin) = (
            std::rc::Rc::clone(&asked),
            std::rc::Rc::clone(&lifted),
            std::rc::Rc::clone(&origin),
        );
        Callback::new(move |press: PointerPress| {
            asked.set(false);
            lifted.set(false);
            origin.set(Some(press));
            on_press.call(press);
        })
    };
    let dragged = Callback::new(move |press: PointerPress| {
        if lifted.get() {
            return;
        }
        if !asked.replace(true)
            && let Some(driver) = crate::drag::driver()
            && !driver.carrying()
            && let Some(mut carry) = drag.call(press)
        {
            // Unless the carry says otherwise, the item is this node, held where it was pressed.
            if carry.item.is_none() {
                let at = origin.get().unwrap_or(press);
                carry.item = Some(crate::types::Rect::from_min_size(
                    crate::types::Pos2::ZERO - at.local.to_vec2(),
                    at.size,
                ));
            }
            lifted.set(true);
            driver.lift(carry, crate::drag::DragPoint::of(press));
            return;
        }
        on_drag.call(press);
    });
    (pressed, dragged)
}
