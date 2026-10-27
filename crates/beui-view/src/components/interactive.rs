use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::ImeCursor;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::CursorIcon;
use crate::types::ImeEvent;
use crate::types::ImeText;
use crate::types::KeyPress;
use crate::types::PasteKind;
use crate::types::Pasted;
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
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    #[prop(default = Vec::new())] pastes: Prop<Vec<PasteKind>>,
    on_paste: Callback<Pasted>,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    #[prop(default = None)] cursor: Prop<Option<CursorIcon>>,
    #[prop(default = false)] pointer_locked: Prop<bool>,
    #[prop(default = false)] key_active: Prop<bool>,
    #[prop(default = Area::Nowhere)] capture_presses: Prop<Area>,
    #[prop(default = false)] repeat_drag: Prop<bool>,
    #[prop(default = TouchDrags::None)] touch_drags: Prop<TouchDrags>,
    #[prop(default = true)] claims_touch: Prop<bool>,
    #[prop(default = Area::Nowhere)] drawing: Prop<Area>,
    #[prop(default = false)] every_sample: Prop<bool>,
    #[prop(default = false)] grabs_pointer: Prop<bool>,
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
