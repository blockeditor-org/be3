use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::ClickCallback;
use crate::reactive::Prop;
use beui_core::base::list::Direction;
use beui_core::clipboard::PasteKind;
use beui_core::clipboard::Pasted;
use beui_core::focus::ImeCursor;
use beui_core::geometry::Pos2;
use beui_core::geometry::Vec2;
use beui_core::input::CursorIcon;
use beui_core::input::ImeEvent;
use beui_core::input::ImeText;
use beui_core::input::KeyPress;
use beui_core::input::Modifiers;
use beui_core::input::PointerInput;
use beui_core::input::PointerPress;
use beui_core::input::WheelInput;
use beui_core::interact::forward::ForwardedInput;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Interactive(
    #[prop(default = false)] focusable: bool,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = false)] yields_tab_stop: bool,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] ime: Prop<bool>,
    #[prop(default = true)] keyboard_on_focus: bool,
    #[prop(default = None)] keyboard_asks: Prop<Option<u32>>,
    #[prop(default = true)] ime_keyboard: Prop<bool>,
    #[prop(default = None)] ime_cursor: Prop<Option<ImeCursor>>,
    #[prop(default = None)] ime_text: Prop<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    #[prop(default = true)] takes_text: bool,
    #[prop(default = Vec::new())] pastes: Prop<Vec<PasteKind>>,
    on_paste: Callback<Pasted>,
    on_ime: Callback<ImeEvent>,
    on_key: Callback<KeyPress, bool>,
    on_motion: Callback<Vec2>,
    #[prop(default = None)] cursor: Prop<Option<CursorIcon>>,
    #[prop(default = false)] pointer_locked: Prop<bool>,
    #[prop(default = false)] key_active: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = false)] repeat_drag: Prop<bool>,
    #[prop(default = false)] touch_drags: Prop<bool>,
    #[prop(default = None)] touch_drag_axis: Prop<Option<Direction>>,
    #[prop(default = true)] claims_touch: Prop<bool>,
    #[prop(default = false)] drawing: Prop<bool>,
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
    on_drag: Callback<PointerPress>,
    on_pointer: Callback<PointerInput, bool>,
    on_wheel: Callback<WheelInput, bool>,
    capture_at: Callback<Pos2, bool>,
    gestures_at: Callback<Pos2, bool>,
    hit_at: Callback<Pos2, bool>,
    drawing_at: Callback<Pos2, bool>,
    on_forward: Callback<ForwardedInput>,
    #[prop(default = false)] inhibits_shortcuts: Prop<bool>,
    #[prop(default = false)] clipboard_keys: Prop<bool>,
    #[prop(default = true)] forwards_wheel: Prop<bool>,
    #[prop(default = None)] claim_modifiers: Prop<Option<Modifiers>>,
    claim_at: Callback<(Pos2, Modifiers), bool>,
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
        keyboard_on_focus,
        keyboard_asks,
        ime_keyboard,
        ime_cursor,
        ime_text,
        on_focus_change,
        on_activate_change,
        on_activate,
        on_step,
        on_text,
        takes_text,
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
        touch_drag_axis,
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
        on_drag,
        on_pointer,
        on_wheel,
        capture_at,
        gestures_at,
        hit_at,
        drawing_at,
        on_forward,
        inhibits_shortcuts,
        clipboard_keys,
        forwards_wheel,
        claim_modifiers,
        claim_at,
        on_press_outside,
        children,
    )
}
