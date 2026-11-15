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
use beui_macros::{component, view};

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
    let (on_press, on_drag, children) = match drag {
        Some(drag) => carrying(drag, on_press, on_drag, children),
        None => (on_press, on_drag, children),
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

type Preview = std::rc::Rc<dyn Fn() -> NodeId>;

/// Wraps a press's callbacks so the first drag of each press asks `drag` for a carry and
/// lifts it onto the tree's drag driver; once lifted, the driver follows the pointer and the
/// press hears no more drags. While the carry floats, the children are drawn where the item
/// floats, raised above their siblings (a scroll around them still clips them), unless the
/// carry gives a `preview`, which floats in a layer of its own instead. If this node goes
/// while its carry is held, the carry is put down, dropping nothing.
fn carrying(
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    on_press: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    children: Option<Child>,
) -> (Callback<PointerPress>, Callback<PointerPress>, Option<Child>) {
    use crate::reactive::{Get, Set, create_memo, create_signal, on_cleanup};
    use std::cell::RefCell;
    use crate::components::overlay::Overlay;
    use crate::primitives::{Frame, List};
    use crate::types::{LayerLevel, LayerPolicy, Placement, Rect};
    use std::cell::Cell;
    use std::rc::Rc;

    let asked = Rc::new(Cell::new(false));
    // Whether this press lifted a carry: the board follows its drags from then on, so they
    // are not this node's to hear.
    let lifted = Rc::new(Cell::new(false));
    let origin = Rc::new(Cell::new(None::<PointerPress>));
    let mine = create_signal(None::<crate::drag::CarryId>);
    let preview = Rc::new(RefCell::new(None::<Preview>));
    let previewed = create_signal(false);
    let pressed = {
        let (asked, lifted, origin) = (Rc::clone(&asked), Rc::clone(&lifted), Rc::clone(&origin));
        Callback::new(move |press: PointerPress| {
            asked.set(false);
            lifted.set(false);
            origin.set(Some(press));
            on_press.call(press);
        })
    };
    let dragged = {
        let (origin, preview) = (Rc::clone(&origin), Rc::clone(&preview));
        Callback::new(move |press: PointerPress| {
            if lifted.get() {
                return;
            }
            if !asked.replace(true)
                && let Some(driver) = crate::drag::driver()
                && !driver.carrying()
                && let Some(mut carry) = drag.call(press)
            {
                // Unless the carry says otherwise, the item is this node, held where it was
                // pressed.
                if carry.item.is_none() {
                    let at = origin.get().unwrap_or(press);
                    carry.item = Some(Rect::from_min_size(
                        Pos2::ZERO - at.local.to_vec2(),
                        at.size,
                    ));
                }
                let floats = carry.preview.clone();
                if let Some(id) = driver.lift(carry, crate::drag::DragPoint::of(press)) {
                    lifted.set(true);
                    previewed.set(floats.is_some());
                    *preview.borrow_mut() = floats;
                    mine.set(Some(id));
                }
                return;
            }
            on_drag.call(press);
        })
    };
    on_cleanup(move || {
        let id = mine.is_alive().then(|| mine.get_untracked()).flatten();
        if let (Some(id), Some(driver)) = (id, crate::drag::driver()) {
            driver.put_down(id);
        }
    });
    // The frame this node's carry shows, while it holds one.
    let frame = create_memo(move || {
        let id = mine.get()?;
        let driver = crate::drag::driver()?;
        (driver.carry() == Some(id)).then(|| driver.frame()).flatten()
    });
    let floating = create_memo(move || frame.get().is_some_and(|frame| frame.floating));
    let raised = create_memo(move || floating.get() && !previewed.get());
    // Where the children are laid out, measured once the wrapper below is built.
    let measured = create_signal(None::<crate::reactive::ReadSignal<Option<crate::measure::Measured>>>);
    let placed = create_memo(move || {
        measured
            .get()
            .and_then(|measured| measured.get())
            .map(|measured| measured.rect.min)
    });
    let offset = create_memo(move || match (raised.get(), frame.get(), placed.get()) {
        (true, Some(frame), Some(placed)) => frame.item.min - placed,
        _ => Vec2::ZERO,
    });
    let anchor = create_memo(move || {
        crate::reactive::OverlayAnchor::Point(frame.get().map_or(Pos2::ZERO, |frame| frame.item.min))
    });
    let window = create_memo(move || origin_window(&origin));
    let open = create_memo(move || floating.get() && previewed.get());
    let wrapped = view! {
        <Frame raised={raised}>
            <List spacing=0.0>
                <Frame translate={offset} children={children} />
                <Overlay
                    anchor={anchor}
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Inert
                    level=LayerLevel::Top
                    traps_focus=false
                    shown_in={window}
                    open={open}
                >
                    || {
                        let shown = preview.borrow().clone();
                        match shown {
                            Some(preview) => preview(),
                            None => view! { <Frame /> },
                        }
                    }
                </Overlay>
            </List>
        </Frame>
    };
    measured.set(Some(crate::measure::measure(wrapped)));
    (pressed, dragged, Some(wrapped.into()))
}

fn origin_window(origin: &std::rc::Rc<std::cell::Cell<Option<PointerPress>>>) -> Option<crate::types::WindowId> {
    origin.get().map(|press| press.window)
}
