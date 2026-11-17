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
    drag: Option<Func<PointerPress, Option<crate::drag::Carry>>>,
    /// Whether a finger's long press lifts `drag`'s carry. Off by default: a held finger is
    /// a secondary press, the platform's way to a menu, and a finger lifts the carry by
    /// dragging along `touch_drags`. On for a source with no menu that a finger moves by
    /// holding it first (a card on a board, a row in a list that scrolls the other way).
    #[prop(default = false)]
    lifts_on_long_press: bool,
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
    // A source that also activates keeps Space and Enter for that (decisions/
    // beui_activating_drag_sources_keep_their_keys.md).
    let activates = !on_activate.is_empty() || !on_activate_change.is_empty();
    let source = crate::reactive::NodeRef::new();
    let (on_press, on_drag, on_long_press_drag, on_key, children) = match drag {
        Some(drag) => {
            let carry = carrying(
                drag,
                (focusable, activates, source),
                on_press,
                on_drag,
                on_long_press_drag,
                lifts_on_long_press,
            );
            let Face {
                raised,
                offset,
                said,
                anchor,
                open,
                preview,
            } = carry.face.clone();
            let face = view! {
                <CarriedFace raised offset said anchor open preview children />
            };
            let on_key = (carry.finish)(face, on_key);
            (
                carry.pressed,
                carry.dragged,
                carry.long_pressed,
                on_key,
                Some(face.into()),
            )
        }
        None => (on_press, on_drag, on_long_press_drag, on_key, children),
    };
    let node = crate::core::interactive(
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
    );
    source.fill(node);
    node
}

type Preview = std::rc::Rc<dyn Fn() -> NodeId>;

/// What `CarriedFace` draws from.
#[derive(Clone)]
struct Face {
    raised: crate::reactive::Memo<bool>,
    offset: crate::reactive::Memo<Vec2>,
    said: crate::reactive::Memo<Option<String>>,
    anchor: crate::reactive::Memo<crate::reactive::OverlayAnchor>,
    open: crate::reactive::Memo<bool>,
    preview: std::rc::Rc<std::cell::RefCell<Option<Preview>>>,
}

/// A source's carrying, before its face is built: the press callbacks, what the face draws
/// from, and, once the face is built, the key callback (which measures and lifts the face).
struct Carrying {
    pressed: Callback<PointerPress>,
    dragged: Callback<PointerPress>,
    long_pressed: Callback<PointerPress>,
    face: Face,
    finish: Box<dyn FnOnce(NodeId, Callback<KeyPress, bool>) -> Callback<KeyPress, bool>>,
}

/// Wraps a press's callbacks so the first drag of each press asks `drag` for a carry and
/// lifts it onto the tree's drag driver; once lifted, the driver follows the pointer and the
/// press hears no more drags. While the carry is held, the children are drawn where its frame
/// puts the item, raised above their siblings (a scroll around them still clips them), unless
/// its `face` is a `preview`, which floats in a layer of its own, pinned by its top left corner
/// and in this node's window (a carry crossing windows waits for the frame to name one,
/// plans/declarative_dragging.md step 6), or `Site` (`Carry::drawn_by_site`), when nothing is
/// drawn here for it. The source hears the carry keys while the focus is on it or within it
/// (`carry_keys`, `step_keys`, `held_keys`). If this node goes while its carry is held, the carry is put down,
/// dropping nothing.
fn carrying(
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    (focusable, activates, source): (bool, bool, crate::reactive::NodeRef),
    on_press: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    on_long_press_drag: Callback<PointerPress>,
    lifts_on_long_press: bool,
) -> Carrying {
    use crate::reactive::{Get, Set, create_memo, create_signal, on_cleanup};
    use crate::types::Rect;
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::rc::Rc;

    let asked = Rc::new(Cell::new(false));
    // Whether this press lifted a carry: the board follows its drags from then on, so they
    // are not this node's to hear.
    let lifted = Rc::new(Cell::new(false));
    let origin = Rc::new(Cell::new(None::<PointerPress>));
    let mine = create_signal(None::<crate::drag::CarryId>);
    // Whether the keyboard lifted this node's carry: only then are its keys the carry's.
    let by_keys = create_signal(false);
    let preview = Rc::new(RefCell::new(None::<Preview>));
    // Whether the carry's face is not the source's: then the children are not displaced.
    let previewed = create_signal(false);
    // Whether this node floats the carry's own preview.
    let floats_here = create_signal(false);
    let pressed = {
        let (asked, lifted, origin) = (Rc::clone(&asked), Rc::clone(&lifted), Rc::clone(&origin));
        Callback::new(move |press: PointerPress| {
            asked.set(false);
            lifted.set(false);
            origin.set(Some(press));
            on_press.call(press);
        })
    };
    // Takes up what a lifted carry draws: its preview floats here, and the children are not
    // displaced unless the source is its face.
    let show_face = {
        let preview = Rc::clone(&preview);
        Rc::new(move |face: crate::drag::CarryFace| {
            use crate::drag::CarryFace;
            previewed.set(!matches!(face, CarryFace::Source));
            let floats = match face {
                CarryFace::Preview(preview) => Some(preview),
                CarryFace::Source | CarryFace::Site => None,
            };
            floats_here.set(floats.is_some());
            *preview.borrow_mut() = floats;
        })
    };
    // Asks `drag` for a carry at `press` and lifts it; whether `drag` gave one.
    let lift_at = {
        let (origin, show_face, lifted) = (
            Rc::clone(&origin),
            Rc::clone(&show_face),
            Rc::clone(&lifted),
        );
        Rc::new(move |press: PointerPress| -> bool {
            let Some(driver) = crate::drag::driver().filter(|driver| !driver.carrying()) else {
                return false;
            };
            let Some(mut carry) = drag.call(press) else {
                return false;
            };
            // Unless the carry says otherwise, the item is this node, held where it was
            // pressed.
            if carry.item.is_none() {
                let at = origin.get().unwrap_or(press);
                carry.item = Some(Rect::from_min_size(
                    Pos2::ZERO - at.local.to_vec2(),
                    at.size,
                ));
            }
            let face = carry.face().clone();
            if let Some(id) = driver.lift(carry, crate::drag::DragPoint::of(press)) {
                lifted.set(true);
                show_face(face);
                by_keys.set(false);
                mine.set(Some(id));
            }
            true
        })
    };
    // The first drag past the threshold asks for a carry; the moves before it are not yet a
    // drag of either kind, and every move of a press `drag` declined goes on as the source's
    // own.
    let dragged = {
        let (lift_at, lifted, origin) =
            (Rc::clone(&lift_at), Rc::clone(&lifted), Rc::clone(&origin));
        Callback::new(move |press: PointerPress| {
            if lifted.get() {
                return;
            }
            let threshold = match press.touch {
                true => crate::types::TOUCH_DRAG_THRESHOLD,
                false => crate::types::DRAG_THRESHOLD,
            };
            // Once a press has crossed the threshold it is a drag until it ends, however near
            // its start it comes back.
            let past = asked.get()
                || origin
                    .get()
                    .is_none_or(|origin| (press.pos - origin.pos).length() >= threshold);
            if !past || (!asked.replace(true) && lift_at(press)) {
                return;
            }
            on_drag.call(press);
        })
    };
    // A finger's long press lifts too, as a drag does for the mouse; a long press `drag`
    // declines goes on as the source's own.
    let long_pressed = match lifts_on_long_press {
        true => Callback::new(move |press: PointerPress| {
            if !lift_at(press) {
                on_long_press_drag.call(press);
            }
        }),
        false => on_long_press_drag,
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
        (driver.carry() == Some(id))
            .then(|| driver.frame())
            .flatten()
    });
    let floating = create_memo(move || frame.get().is_some_and(|frame| frame.floating));
    // The item is drawn where the frame says while it is carried: floating under the
    // pointer, or on the place chosen (where a site that already draws it there, a list
    // shown in its new order, leaves no offset).
    let raised = create_memo(move || frame.get().is_some() && !previewed.get());
    // Where the children are laid out, measured once the wrapper below is built.
    let measured =
        create_signal(None::<crate::reactive::ReadSignal<Option<crate::measure::Measured>>>);
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
        crate::reactive::OverlayAnchor::Point(
            frame.get().map_or(Pos2::ZERO, |frame| frame.item.min),
        )
    });
    let open = create_memo(move || floating.get() && floats_here.get());
    // What this node's carry last said, for a screen reader.
    let said = create_memo(move || {
        let id = mine.get()?;
        let (said_by, said) = crate::drag::driver()?.said()?;
        (said_by == id).then_some(said)
    });
    let keyboard_lift = {
        let show_face = Rc::clone(&show_face);
        Rc::new(
            move |id: crate::drag::CarryId, face: crate::drag::CarryFace| {
                show_face(face);
                by_keys.set(true);
                mine.set(Some(id));
            },
        ) as Rc<dyn Fn(crate::drag::CarryId, crate::drag::CarryFace)>
    };
    let face = Face {
        raised,
        offset,
        said,
        anchor,
        open,
        preview: Rc::clone(&preview),
    };
    let finish = Box::new(move |wrapped: NodeId, on_key: Callback<KeyPress, bool>| {
        use crate::reactive::{KeyAt, KeyReveal};
        use crate::types::KeyPhase;
        let laid_out = crate::measure::measure(wrapped);
        measured.set(Some(laid_out));
        let at = (wrapped, laid_out);
        // The source hears the carry keys while it or a node within it has the focus (a tab
        // bar's row, whose tab is the focusable button inside it), and offers "Move" in the
        // palette then, in an action scope of its own.
        {
            let lift = Rc::clone(&keyboard_lift);
            crate::reactive::child_scope(move || {
                crate::actions::action_scope(&source);
                crate::actions::Action::new("drag.move", "Move", move || {
                    lift_by_keys(drag, at, &lift);
                })
                .register();
            });
        }
        // While the keyboard holds this source's carry its keys come first, before the node
        // within it that has the focus (a tab's own arrows).
        crate::reactive::on_key(
            KeyAt::Node(source),
            KeyPhase::Capture,
            KeyReveal::Focus,
            move |event| {
                event
                    .press()
                    .is_some_and(|press| by_keys.get_untracked() && held_keys(press, mine))
            },
        );
        match focusable {
            true => Callback::new(move |press: KeyPress| {
                carry_keys(press, activates, drag, at, &keyboard_lift) || on_key.call(press)
            }),
            // Focused within, it moves one place with Ctrl+Shift and an arrow once the
            // focused node has turned the key down.
            false => {
                crate::reactive::on_key(
                    KeyAt::Node(source),
                    KeyPhase::Bubble,
                    KeyReveal::Focus,
                    move |event| {
                        event
                            .press()
                            .is_some_and(|press| step_keys(press, drag, at, &keyboard_lift))
                    },
                );
                on_key
            }
        }
    });
    Carrying {
        pressed,
        dragged,
        long_pressed,
        face,
        finish,
    }
}

/// Keys on a focusable source given `drag`, while it has the focus and the keyboard holds no
/// carry of it (`held_keys` takes those). Space lifts its carry with the keyboard, unless the
/// source also activates, when Space and Enter stay its own; Ctrl+Shift and an arrow move it one
/// place (`step_keys`). Whether the key was taken.
fn carry_keys(
    press: KeyPress,
    activates: bool,
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    at: (
        NodeId,
        crate::reactive::ReadSignal<Option<crate::measure::Measured>>,
    ),
    lifted: &std::rc::Rc<dyn Fn(crate::drag::CarryId, crate::drag::CarryFace)>,
) -> bool {
    use crate::types::Key;
    let modifiers = press.modifiers;
    if !press.pressed || modifiers.alt || modifiers.logo {
        return false;
    }
    if step_keys(press, drag, at, lifted) {
        return true;
    }
    if press.key != Key::Space || activates || modifiers.ctrl || modifiers.shift {
        return false;
    }
    lift_by_keys(drag, at, lifted)
}

fn arrow(key: crate::types::Key) -> Option<Vec2> {
    use crate::types::Key;
    match key {
        Key::ArrowUp => Some(Vec2::new(0.0, -1.0)),
        Key::ArrowDown => Some(Vec2::new(0.0, 1.0)),
        Key::ArrowLeft => Some(Vec2::new(-1.0, 0.0)),
        Key::ArrowRight => Some(Vec2::new(1.0, 0.0)),
        _ => None,
    }
}

/// Ctrl+Shift and an arrow on a source: its carry lifted, stepped to the nearest place that
/// way and dropped at once; one with no place that way is put down, dropping nothing. Whether
/// the key was taken: a press that lifts nothing is the source's own.
fn step_keys(
    press: KeyPress,
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    at: (
        NodeId,
        crate::reactive::ReadSignal<Option<crate::measure::Measured>>,
    ),
    lifted: &std::rc::Rc<dyn Fn(crate::drag::CarryId, crate::drag::CarryFace)>,
) -> bool {
    use crate::types::{WindowId, input::PointerId};
    let modifiers = press.modifiers;
    if !press.pressed || !modifiers.ctrl || !modifiers.shift || modifiers.alt || modifiers.logo {
        return false;
    }
    let Some(toward) = arrow(press.key) else {
        return false;
    };
    let Some(driver) = crate::drag::driver() else {
        return false;
    };
    if !lift_by_keys(drag, at, lifted) {
        return false;
    }
    let flat = |_| (Vec::new(), WindowId::MAIN);
    let stepped = driver.step(toward);
    driver.release(PointerId::Keyboard, !stepped, &flat);
    true
}

/// Keys while the keyboard holds this source's carry (a pointer's carry leaves keys to the
/// focused node): the arrows step it between the places it
/// can land, Space or Enter drops it there, and Escape or Tab puts it down where it was (Tab
/// does not move focus away from a carry in hand). Whether the key was taken.
fn held_keys(press: KeyPress, mine: crate::reactive::Signal<Option<crate::drag::CarryId>>) -> bool {
    use crate::reactive::Get;
    use crate::types::{Key, WindowId, input::PointerId};
    let modifiers = press.modifiers;
    if !press.pressed || modifiers.alt || modifiers.logo || modifiers.ctrl {
        return false;
    }
    let Some(driver) = crate::drag::driver() else {
        return false;
    };
    let held = mine.is_alive()
        && mine
            .get_untracked()
            .is_some_and(|id| driver.carry() == Some(id));
    if !held {
        return false;
    }
    let flat = |_| (Vec::new(), WindowId::MAIN);
    if let Some(toward) = arrow(press.key) {
        driver.step(toward);
        return true;
    }
    match press.key {
        Key::Space | Key::Enter => {
            driver.release(PointerId::Keyboard, false, &flat);
            true
        }
        Key::Escape | Key::Tab => {
            driver.release(PointerId::Keyboard, true, &flat);
            true
        }
        _ => false,
    }
}

/// Lifts this source's carry with the keyboard: pressed at its middle, the item its own
/// rect. Whether it lifted.
fn lift_by_keys(
    drag: Func<PointerPress, Option<crate::drag::Carry>>,
    (node, laid_out): (
        NodeId,
        crate::reactive::ReadSignal<Option<crate::measure::Measured>>,
    ),
    lifted: &std::rc::Rc<dyn Fn(crate::drag::CarryId, crate::drag::CarryFace)>,
) -> bool {
    use crate::reactive::Get;
    use crate::types::{Rect, input::PointerId};
    let Some(driver) = crate::drag::driver().filter(|driver| !driver.carrying()) else {
        return false;
    };
    let Some(rect) = laid_out.get_untracked().map(|measured| measured.rect) else {
        return false;
    };
    let at = PointerPress {
        window: crate::reactive::window_of(node),
        pos: rect.center(),
        local: Pos2::ZERO + rect.size() / 2.0,
        size: rect.size(),
        fraction: Vec2::new(0.5, 0.5),
        clicks: 0,
        modifiers: crate::types::Modifiers::NONE,
        touch: false,
        pen: None,
        predicted: None,
        time: crate::reactive::now(),
        pointer: PointerId::Keyboard,
    };
    let Some(mut carry) = drag.call(at) else {
        return false;
    };
    if carry.item.is_none() {
        carry.item = Some(Rect::from_min_size(
            Pos2::ZERO - rect.size() / 2.0,
            rect.size(),
        ));
    }
    let face = carry.face().clone();
    match driver.lift(carry, crate::drag::DragPoint::of(at)) {
        Some(id) => {
            lifted(id, face);
            true
        }
        None => false,
    }
}

/// What an `Interactive` given `drag` wraps its children in: drawn raised and offset by
/// `offset` while carried, a live region saying what the carry does, and the overlay its
/// `preview` floats in.
#[component]
fn CarriedFace(
    raised: crate::reactive::Memo<bool>,
    offset: crate::reactive::Memo<Vec2>,
    said: crate::reactive::Memo<Option<String>>,
    anchor: crate::reactive::Memo<crate::reactive::OverlayAnchor>,
    open: crate::reactive::Memo<bool>,
    preview: std::rc::Rc<std::cell::RefCell<Option<Preview>>>,
    children: Option<Child>,
) -> NodeId {
    use crate::components::overlay::Overlay;
    use crate::primitives::{Frame, List};
    use crate::types::{LayerLevel, LayerPolicy, Placement};
    view! {
        <Frame raised={raised}>
            <List spacing=0.0>
                <CarryStatus said={said} />
                <Frame translate={offset} children={children} />
                <Overlay
                    anchor={anchor}
                    placement=Placement::At
                    policy=LayerPolicy::Inert
                    level=LayerLevel::Top
                    traps_focus=false
                    open={open}
                >|| {
                    let shown = preview.borrow().clone();
                    match shown {
                        Some(preview) => preview(),
                        None => view! {
                            <Frame />
                        },
                    }
                }</Overlay>
            </List>
        </Frame>
    }
}

/// A live region saying what a carry does, for a screen reader.
#[component]
fn CarryStatus(said: crate::reactive::Memo<Option<String>>) -> NodeId {
    use crate::reactive::Get;
    crate::reactive::component_accessibility(crate::reactive::create_memo(move || {
        let mut node = accesskit::Node::new(accesskit::Role::Status);
        node.set_live(accesskit::Live::Polite);
        if let Some(said) = said.get() {
            node.set_label(said);
        }
        node
    }));
    use crate::primitives::Frame;
    view! {
        <Frame />
    }
}
