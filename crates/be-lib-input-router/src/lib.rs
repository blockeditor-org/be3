//! The forwarding router (plans/host_as_platform_adapter.md, Input): the rules beui applies to
//! its forwarding catchers (guides/beui_keyboard.md, One input path), for surfaces rather than
//! nodes. Given a hit test, it routes a window's raw input to the surface each event is for, in
//! that surface's own coordinates: a press captures the pointer until every button lifts, hover
//! follows the surface under the pointer, wheel and pinch go to the outermost placer whose
//! surface grants it the gesture there, a finger landing for an outer claim takes the earlier
//! fingers with it, keys go to the focus except the chords the compositor intercepts (a key's
//! release going where its press went), and the
//! pointer lock and a drag between surfaces follow the focus and the pointer.
//!
//! It knows nothing of how surfaces are found: the host hands it a 2D test over its layer list
//! (be-lib-layers), Space a ray test, and an app routing to children it places a test of its
//! own. beui-core's routing inside a document stays its own.

use std::fmt::Debug;

use be_lib_geometry::Pos2;
use be_lib_ui_types::input::{
    Input, Key, KeyChord, KeyPress, Modifiers, PenPhase, PointerButton, TouchId, TouchPhase,
    TouchpadGesture,
};

/// How far, in points, the pointer may move while a wheel or held gesture stays latched to
/// what it began on: a finger landing on a touchpad jitters the pointer this much. beui's
/// latch of a wheel to the node that took it (beui-core's `interact`) uses it too.
pub const WHEEL_LATCH_SLOP: f32 = 2.0;

/// What the router asks of the surfaces of one window, however they are found.
pub trait HitTest {
    type Target: Copy + Eq + Debug;

    /// The surface a pointer at `at`, in the window's points, lands on: the topmost that takes
    /// it there. A surface that captures presses (a scrim, a popup) and does not take this point
    /// hands it to its placer, which dismisses it, and nothing under it gets it.
    fn hit(&self, at: Pos2) -> Option<Self::Target>;

    /// `at` in `target`'s own coordinates, wherever `target` is (a captured pointer leaves it).
    /// `None` where `target` is gone.
    fn local(&self, target: Self::Target, at: Pos2) -> Option<Pos2>;

    /// The surface that placed `target`, whose camera takes gestures over it first.
    fn placer(&self, target: Self::Target) -> Option<Self::Target>;

    /// Whether `target` grants its placer wheel, pinch and pan-drag at `at`: its gesture area.
    fn grants_gestures(&self, target: Self::Target, at: Pos2) -> bool;

    /// Whether a press on `target` gives it the keyboard (popups and the on-screen keyboard
    /// never take it).
    fn takes_focus(&self, target: Self::Target) -> bool;

    /// Whether a press at `at` pans the camera there rather than pressing anything: it goes to
    /// no surface, even one the pointer holds. beui answers it for a middle press over a gesture
    /// area; the layer list's test will answer it from its layers' gesture areas
    /// (plans/host_as_platform_adapter.md, step 4f).
    fn pans(&self, _at: Pos2) -> bool {
        false
    }
}

/// What the router delivers.
#[derive(Clone, Debug, PartialEq)]
pub enum Routed<T> {
    /// Input for `target`, with its positions in `target`'s own coordinates.
    Input { target: T, input: Input },
    /// A drag the host carries between surfaces (`Router::start_drag`).
    Drag { target: T, drag: Drag<T> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drag<T> {
    /// The carry is over `target` at this point, in its coordinates.
    Over(Pos2),
    /// The carry left `target`, or ended elsewhere.
    Left,
    /// The carry was let go over `target` at this point: the host hands it the payload `from`
    /// started the drag with.
    Dropped { at: Pos2, from: T },
}

/// Which pointer an event is of: each holds its own presses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Mouse,
    Pen,
    Touch(TouchId),
    /// The keyboard carrying a dragged item, which no platform reports as a pointer.
    Keyboard,
    /// A pointer another program or window relays as points (a dock bar an app's grip
    /// carries), which no platform reports either.
    Relayed,
}

impl Pointer {
    /// Whether a press of this pointer holds the surface it began on for the pointer's other
    /// buttons, and keeps the surface from hearing it left while it is pressed: a mouse's and a
    /// pen's do; each finger is a pointer of its own.
    fn holds(self) -> bool {
        !matches!(self, Self::Touch(_))
    }

    /// Whether a surface it is pressed on keeps hearing it as over it.
    fn keeps_hover(self) -> bool {
        matches!(self, Self::Mouse | Self::Pen)
    }
}

/// Whether a gesture (a held scroll, a touchpad swipe or pinch) begins, goes on or ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Began,
    Moved,
    Ended,
}

/// One button of one pointer held down, and the surface it was pressed for (`None`: the press
/// landed on no surface, so the pointer goes to none until it lifts).
#[derive(Clone, Copy, Debug)]
struct Press<T> {
    pointer: Pointer,
    button: PointerButton,
    target: Option<T>,
    at: Pos2,
}

#[derive(Clone, Copy, Debug)]
struct Carry<T> {
    from: T,
    over: Option<T>,
}

/// The state of one window's routing: who holds each pointer's presses, what is under the
/// pointer and what heard it, the focus, the lock, a latched gesture and a drag.
///
/// Its rules come in two layers. `route` takes a window's raw `Input` and routes all of it,
/// as the host routes between surfaces. The steps it is built from (`press`, `moved`, `release`,
/// `cancel`, `gesture`, `entered`, `hover`, `focus`) are public too, for a caller that turns
/// raw input into its own pointer events first and asks only who each is for: beui-core
/// routing to its forwarding catchers (guides/beui_keyboard.md, One input path).
#[derive(Debug)]
pub struct Router<T> {
    compositor: Option<T>,
    intercepted: Vec<KeyChord>,
    modifiers: Modifiers,
    pointer: Option<Pos2>,
    under: Option<T>,
    entered: Vec<T>,
    presses: Vec<Press<T>>,
    /// A held gesture's surface (`None`: no surface's), and where the pointer was when it began.
    gesture: Option<(Option<T>, Pos2)>,
    focused: Option<T>,
    locked: Option<T>,
    /// Each key held down, and the surface its press went to (`None`: none), which hears its
    /// release wherever the focus has gone since.
    keys: Vec<(Key, Option<T>)>,
    carry: Option<Carry<T>>,
}

impl<T> Default for Router<T> {
    /// A router with no compositor of its own: intercepted chords and system gestures go
    /// nowhere, for a caller that handles them before routing (beui-core).
    fn default() -> Self {
        Self {
            compositor: None,
            intercepted: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: None,
            under: None,
            entered: Vec::new(),
            presses: Vec::new(),
            gesture: None,
            focused: None,
            locked: None,
            keys: Vec::new(),
            carry: None,
        }
    }
}

impl<T: Copy + Eq + Debug> Router<T> {
    /// A router whose intercepted chords and system gestures (back) go to `compositor`.
    pub fn new(compositor: T) -> Self {
        Self {
            compositor: Some(compositor),
            ..Self::default()
        }
    }

    /// The chords the compositor takes before the focus hears them
    /// (`WindowRequests::intercepted_keys` today).
    pub fn intercept(&mut self, chords: Vec<KeyChord>) {
        self.intercepted = chords;
    }

    pub fn focused(&self) -> Option<T> {
        self.focused
    }

    /// The surface under the pointer, whose cursor shape the window shows.
    pub fn hovered(&self) -> Option<T> {
        self.under
    }

    /// The modifier keys held, as the window last said: what a drop of a carry does.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Whether something is carried between surfaces (`start_drag`), until it is dropped or
    /// put down.
    pub fn carrying(&self) -> bool {
        self.carry.is_some()
    }

    /// The surface the mouse's press is held for, until every button lifts.
    pub fn pressed(&self) -> Option<T> {
        self.held(Pointer::Mouse)
    }

    pub fn locked(&self) -> Option<T> {
        self.locked
    }

    /// The surface `pointer`'s presses are held for: the first of them that landed on one.
    pub fn held(&self, pointer: Pointer) -> Option<T> {
        self.presses
            .iter()
            .filter(|press| press.pointer == pointer)
            .find_map(|press| press.target)
    }

    /// Whether `pointer` has a button down, whether or not its press landed on a surface.
    pub fn pressing(&self, pointer: Pointer) -> bool {
        self.presses.iter().any(|press| press.pointer == pointer)
    }

    /// Whether any pointer has a button down or a finger on, whether or not it landed on a
    /// surface: its release has yet to come.
    pub fn holding(&self) -> bool {
        !self.presses.is_empty()
    }

    /// `pointer` presses `button` at `at`: the surface the press is for, held until the button
    /// lifts. A mouse or pen already holding a surface keeps it for its other buttons; a press
    /// that pans the camera under it (`HitTest::pans`) is no surface's, even one already held.
    pub fn press<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        pointer: Pointer,
        button: PointerButton,
        at: Pos2,
    ) -> Option<T> {
        let target = match test.pans(at) {
            true => None,
            false => self
                .held(pointer)
                .filter(|_| pointer.holds())
                .or_else(|| test.hit(at)),
        };
        self.presses
            .retain(|press| !(press.pointer == pointer && press.button == button));
        self.presses.push(Press {
            pointer,
            button,
            target,
            at,
        });
        target
    }

    /// `pointer` moved to `at`: the surface holding its press while it is pressed (none, if
    /// the press landed on none), else the one under it.
    pub fn moved<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        pointer: Pointer,
        at: Pos2,
    ) -> Option<T> {
        // A held gesture whose end never came lets go once the pointer moves past the slop: a
        // touchpad holds it nearly still while it scrolls, and a pointer that rests where it
        // was moves nothing.
        let moved_off = |(_, began): (Option<T>, Pos2)| began.distance(at) > WHEEL_LATCH_SLOP;
        if pointer.keeps_hover() && self.gesture.is_some_and(moved_off) {
            self.gesture = None;
        }
        for press in &mut self.presses {
            if press.pointer == pointer {
                press.at = at;
            }
        }
        match self.pressing(pointer) {
            true => self.held(pointer),
            false => test.hit(at),
        }
    }

    /// `pointer` lifts `button`: the surface the release is for, the one the pointer's other
    /// buttons still hold or else the one this button held. A button it never pressed goes to
    /// what the others hold.
    pub fn release(&mut self, pointer: Pointer, button: PointerButton) -> Option<T> {
        let held = |router: &Self| router.held(pointer).filter(|_| pointer.holds());
        let Some(index) = self
            .presses
            .iter()
            .position(|press| press.pointer == pointer && press.button == button)
        else {
            return held(self);
        };
        let press = self.presses.remove(index);
        held(self).or(press.target)
    }

    /// `pointer`'s presses end unreleased: the surface they were held for, which hears the
    /// cancel. A held gesture ends with them.
    pub fn cancel(&mut self, pointer: Pointer) -> Option<T> {
        self.gesture = None;
        let held = self.held(pointer);
        self.presses.retain(|press| press.pointer != pointer);
        held
    }

    /// A wheel, pinch or touchpad gesture at `at`: the surface it is for. One that begins held
    /// (a touchpad's scroll or swipe) stays with the surface it began on until it ends, the
    /// pointer moves or leaves, a press is cancelled, the window loses the keyboard or the
    /// surface goes (`end_gesture`, `forget`); any other goes to whichever takes it where it is
    /// (`claimant`).
    pub fn gesture<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        at: Pos2,
        phase: Phase,
    ) -> Option<T> {
        match phase {
            Phase::Began => {
                let target = claimant(test, at);
                self.gesture = Some((target, at));
                target
            }
            Phase::Moved => match self.gesture {
                Some((latched, _)) => latched,
                None => claimant(test, at),
            },
            Phase::Ended => match self.gesture.take() {
                Some((latched, _)) => latched,
                None => claimant(test, at),
            },
        }
    }

    /// The surface a held gesture is latched to (`None` inside: to no surface), if one is held.
    pub fn latched(&self) -> Option<Option<T>> {
        self.gesture.map(|(latched, _)| latched)
    }

    /// Ends a held gesture whose end will not come: the window lost the keyboard or the
    /// pointer, or the surface it is latched to went.
    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    /// A pointer that shows hover was delivered to `target`: it hears `PointerLeft` once the
    /// pointer is neither over it nor pressed on it (`hover`).
    pub fn entered(&mut self, target: T) {
        if !self.entered.contains(&target) {
            self.entered.push(target);
        }
    }

    /// The surface under the pointer is now `under`. The surfaces the pointer was delivered to
    /// that it is no longer over, and no mouse or pen press holds, which hear `PointerLeft`.
    pub fn hover(&mut self, under: Option<T>) -> Vec<T> {
        self.under = under;
        let presses = &self.presses;
        let mut left = Vec::new();
        self.entered.retain(|target| {
            let stays = under == Some(*target)
                || presses
                    .iter()
                    .any(|press| press.pointer.keeps_hover() && press.target == Some(*target));
            if !stays {
                left.push(*target);
            }
            stays
        });
        left
    }

    /// Routes one event of the window's raw input.
    pub fn route<H: HitTest<Target = T>>(&mut self, test: &H, input: Input) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        match input {
            Input::Focus(focused) => {
                if !focused {
                    self.unlock_into(&mut out);
                    self.end_gesture();
                    // Every surface lets go of the keys it held when the window loses the
                    // keyboard, so a release that comes after goes nowhere.
                    for (_, holder) in &mut self.keys {
                        *holder = None;
                    }
                }
                self.to_focus(Input::Focus(focused), &mut out);
            }
            Input::Modifiers(modifiers) => {
                self.modifiers = modifiers;
                self.to_focus(Input::Modifiers(modifiers), &mut out);
            }
            // Escape puts a carry down where it was picked up, as a drag board's cancel does: the
            // surface it was over hears it left, the press that carried it ends nowhere, and the
            // key goes to nothing else.
            Input::Key(stroke)
                if self.carry.is_some()
                    && stroke.pressed
                    && !stroke.repeat
                    && stroke.key == Some(Key::Escape) =>
            {
                self.end_carry(&mut out);
                // Its release goes nowhere either.
                self.keys.retain(|(held, _)| *held != Key::Escape);
                self.keys.push((Key::Escape, None));
            }
            Input::Key(stroke) => {
                let target = match (stroke.key, stroke.pressed) {
                    (Some(key), true) => {
                        let press = KeyPress {
                            key,
                            pressed: true,
                            repeat: stroke.repeat,
                            modifiers: self.modifiers,
                        };
                        let held = self.keys.iter().find(|(held, _)| *held == key);
                        match (stroke.repeat, held) {
                            // A repeat goes where its press went, which keeps holding the key,
                            // wherever the focus has moved since.
                            (true, Some((_, holder))) => *holder,
                            _ => {
                                let target =
                                    match self.intercepted.iter().any(|chord| chord.matches(press))
                                    {
                                        true => self.compositor,
                                        false => self.focused,
                                    };
                                self.keys.retain(|(held, _)| *held != key);
                                self.keys.push((key, target));
                                target
                            }
                        }
                    }
                    // A release goes where its press went, the compositor for a chord it took, so
                    // the release of a key that moved the focus (an Escape that closed a dialog,
                    // a Tab) never reaches the surface the focus went to. A release whose press
                    // the router never saw goes to the focus.
                    (Some(key), false) => match self.keys.iter().position(|(held, _)| *held == key)
                    {
                        Some(index) => self.keys.remove(index).1,
                        None => self.focused,
                    },
                    (None, _) => self.focused,
                };
                if let Some(target) = target {
                    out.push(Routed::Input {
                        target,
                        input: Input::Key(stroke),
                    });
                }
            }
            Input::Text(_) | Input::Paste(_) | Input::PastePiece(_) | Input::Ime(_) => {
                self.to_focus(input, &mut out);
            }
            Input::Back(_) | Input::InterceptedKey { .. } | Input::InterceptedTap(_) => {
                if let Some(target) = self.compositor {
                    out.push(Routed::Input { target, input });
                }
            }
            Input::PointerMoved(at) | Input::PointerRested(at) | Input::Predicted(at) => {
                self.pointer = Some(at);
                if self.carry.is_some() {
                    self.carry_over(test, at, &mut out);
                    return out;
                }
                let under = test.hit(at);
                self.left_into(under, &mut out);
                if let Some(target) = self.moved(test, Pointer::Mouse, at) {
                    self.entered(target);
                    deliver(test, target, input, at, &mut out);
                }
            }
            Input::PointerMotion(_) | Input::PointerUnlocked => {
                let unlocked = matches!(input, Input::PointerUnlocked);
                if let Some(target) = self.locked {
                    out.push(Routed::Input { target, input });
                }
                if unlocked {
                    self.locked = None;
                }
            }
            Input::PointerLeft => {
                self.pointer = None;
                self.end_gesture();
                if let Some(carry) = &mut self.carry
                    && let Some(over) = carry.over.take()
                {
                    out.push(Routed::Drag {
                        target: over,
                        drag: Drag::Left,
                    });
                }
                self.left_into(None, &mut out);
            }
            Input::PointerButton {
                pos,
                button,
                pressed,
            } => {
                self.pointer = Some(pos);
                self.button(test, pos, button, pressed, input, &mut out);
            }
            Input::PointersCancelled => {
                self.end_gesture();
                let mut told = Vec::new();
                let mut pointers: Vec<Pointer> =
                    self.presses.iter().map(|press| press.pointer).collect();
                // The mouse's press, then the pen's, then the fingers', each told once.
                pointers.sort_by_key(|pointer| match pointer {
                    Pointer::Mouse => 0,
                    Pointer::Pen => 1,
                    Pointer::Keyboard | Pointer::Relayed => 2,
                    Pointer::Touch(_) => 3,
                });
                pointers.dedup();
                for pointer in pointers {
                    if let Some(target) = self.cancel(pointer)
                        && !told.contains(&target)
                    {
                        told.push(target);
                        out.push(Routed::Input {
                            target,
                            input: Input::PointersCancelled,
                        });
                    }
                }
                self.end_carry(&mut out);
            }
            Input::ScrollStarted | Input::ScrollEnded | Input::Scroll(_) | Input::Zoom(_) => {
                let phase = match input {
                    Input::ScrollStarted => Phase::Began,
                    Input::ScrollEnded => Phase::Ended,
                    _ => Phase::Moved,
                };
                let target = match self.pointer {
                    Some(at) => self.gesture(test, at, phase),
                    None => match phase {
                        Phase::Began => None,
                        Phase::Moved => self.latched().flatten(),
                        Phase::Ended => self.gesture.take().and_then(|(latched, _)| latched),
                    },
                };
                if let Some(target) = target {
                    out.push(Routed::Input { target, input });
                }
            }
            Input::Touchpad(gesture) => {
                let phase = match gesture {
                    TouchpadGesture::SwipeBegan(_)
                    | TouchpadGesture::PinchBegan(_)
                    | TouchpadGesture::HoldBegan(_) => Phase::Began,
                    TouchpadGesture::SwipeEnded(_)
                    | TouchpadGesture::PinchEnded(_)
                    | TouchpadGesture::HoldEnded(_) => Phase::Ended,
                    _ => Phase::Moved,
                };
                let target = self.pointer.and_then(|at| self.gesture(test, at, phase));
                if let Some(target) = target {
                    out.push(Routed::Input {
                        target,
                        input: Input::Touchpad(gesture),
                    });
                }
            }
            Input::Touch {
                id,
                phase,
                pos,
                force,
            } => self.touch(test, id, phase, pos, force, &mut out),
            Input::Pen { phase, pos, pen } => {
                self.pointer = Some(pos);
                let target = match phase {
                    PenPhase::Down(button) => {
                        let target = self.press(test, Pointer::Pen, button, pos);
                        if let Some(target) = target {
                            self.press_focus(test, target, &mut out);
                        }
                        target
                    }
                    PenPhase::Up(button) => {
                        self.release(Pointer::Pen, button).or_else(|| test.hit(pos))
                    }
                    PenPhase::Gone => self.cancel(Pointer::Pen).or_else(|| test.hit(pos)),
                    PenPhase::Move => self.moved(test, Pointer::Pen, pos),
                };
                if let Some(target) = target {
                    deliver(test, target, Input::Pen { phase, pos, pen }, pos, &mut out);
                }
            }
        }
        out
    }

    /// Moves the keyboard to `target` (Tab leaving an app's last field, the palette closing),
    /// telling the surface that had it and the one that gets it.
    pub fn focus(&mut self, target: Option<T>) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        self.move_focus(target, &mut out);
        out
    }

    /// Grants `target` the pointer lock, which only the focused surface may hold. Whether it was
    /// granted.
    pub fn lock(&mut self, target: T) -> bool {
        let granted = self.focused == Some(target);
        if granted {
            self.locked = Some(target);
        }
        granted
    }

    /// Releases the pointer lock, telling the surface that held it.
    pub fn unlock(&mut self) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        self.unlock_into(&mut out);
        out
    }

    /// `from`, holding a press, hands it to its placer (`pan_drag()`): the press ends cancelled in
    /// `from`, and the rest of it, fingers too, goes to the placer.
    pub fn hand_over<H: HitTest<Target = T>>(&mut self, test: &H, from: T) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        let Some(placer) = test.placer(from) else {
            return out;
        };
        if !self.presses.iter().any(|press| press.target == Some(from)) {
            return out;
        }
        out.push(Routed::Input {
            target: from,
            input: Input::PointersCancelled,
        });
        for press in &mut self.presses {
            if press.target != Some(from) {
                continue;
            }
            press.target = Some(placer);
            let input = match press.pointer {
                Pointer::Mouse => Input::PointerButton {
                    pos: press.at,
                    button: press.button,
                    pressed: true,
                },
                Pointer::Touch(id) => Input::Touch {
                    id,
                    phase: TouchPhase::Start,
                    pos: press.at,
                    force: None,
                },
                Pointer::Pen | Pointer::Keyboard | Pointer::Relayed => continue,
            };
            deliver(test, placer, input, press.at, &mut out);
        }
        out
    }

    /// `from`, holding a press, starts carrying something between surfaces: its press ends
    /// cancelled, and until every button lifts the surface under the pointer hears the carry
    /// over it, and the one under the release hears the drop.
    pub fn start_drag<H: HitTest<Target = T>>(&mut self, test: &H, from: T) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        if self.pressed() != Some(from) {
            return out;
        }
        out.push(Routed::Input {
            target: from,
            input: Input::PointersCancelled,
        });
        for press in &mut self.presses {
            if press.pointer == Pointer::Mouse {
                press.target = None;
            }
        }
        self.carry = Some(Carry { from, over: None });
        if let Some(at) = self.pointer {
            self.carry_over(test, at, &mut out);
        }
        out
    }

    /// A surface went while its window stays: as `forget`, but each press it held still holds
    /// its pointer, for no surface, until it lifts, so the release goes nowhere rather than to
    /// whatever lies under it, which never heard the press (a dialog or toast that closed on a
    /// press).
    pub fn went(&mut self, target: T) {
        for press in &mut self.presses {
            if press.target == Some(target) {
                press.target = None;
            }
        }
        self.forget(target);
    }

    /// Forgets a surface that went: it holds nothing any more, and nothing is sent to it. The
    /// pointers it held are free again.
    pub fn forget(&mut self, target: T) {
        let gone = |held: &mut Option<T>| {
            if *held == Some(target) {
                *held = None;
            }
        };
        gone(&mut self.under);
        gone(&mut self.focused);
        gone(&mut self.locked);
        if self.latched() == Some(Some(target)) {
            self.end_gesture();
        }
        self.entered.retain(|entered| *entered != target);
        self.presses.retain(|press| press.target != Some(target));
        for (_, holder) in &mut self.keys {
            if *holder == Some(target) {
                *holder = None;
            }
        }
        if let Some(carry) = &mut self.carry {
            if carry.from == target {
                self.carry = None;
                self.presses.retain(|press| press.pointer != Pointer::Mouse);
            } else if carry.over == Some(target) {
                carry.over = None;
            }
        }
    }

    fn button<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        at: Pos2,
        button: PointerButton,
        pressed: bool,
        input: Input,
        out: &mut Vec<Routed<T>>,
    ) {
        if pressed {
            if self.carry.is_some() {
                self.presses
                    .retain(|press| !(press.pointer == Pointer::Mouse && press.button == button));
                self.presses.push(Press {
                    pointer: Pointer::Mouse,
                    button,
                    target: None,
                    at,
                });
                return;
            }
            let fresh = !self.pressing(Pointer::Mouse);
            if fresh {
                self.left_into(test.hit(at), out);
            }
            let target = self.press(test, Pointer::Mouse, button, at);
            if let Some(target) = target {
                if fresh {
                    self.press_focus(test, target, out);
                }
                self.entered(target);
                deliver(test, target, input, at, out);
            }
            return;
        }
        if let Some(carry) = self.carry {
            self.presses
                .retain(|press| !(press.pointer == Pointer::Mouse && press.button == button));
            if !self.pressing(Pointer::Mouse) {
                let under = test.hit(at);
                if let Some(over) = carry.over
                    && Some(over) != under
                {
                    out.push(Routed::Drag {
                        target: over,
                        drag: Drag::Left,
                    });
                }
                if let Some(target) = under
                    && let Some(local) = test.local(target, at)
                {
                    out.push(Routed::Drag {
                        target,
                        drag: Drag::Dropped {
                            at: local,
                            from: carry.from,
                        },
                    });
                }
                self.carry = None;
                self.left_into(under, out);
            }
            return;
        }
        let pressing = self.pressing(Pointer::Mouse);
        let target = match pressing {
            true => self.release(Pointer::Mouse, button),
            false => test.hit(at),
        };
        if let Some(target) = target {
            deliver(test, target, input, at, out);
        }
        if !self.pressing(Pointer::Mouse) {
            self.left_into(test.hit(at), out);
        }
    }

    fn touch<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        id: TouchId,
        phase: TouchPhase,
        at: Pos2,
        force: Option<f32>,
        out: &mut Vec<Routed<T>>,
    ) {
        let input = Input::Touch {
            id,
            phase,
            pos: at,
            force,
        };
        let finger = Pointer::Touch(id);
        let fingers = |router: &Self| -> Vec<Press<T>> {
            router
                .presses
                .iter()
                .copied()
                .filter(|press| matches!(press.pointer, Pointer::Touch(_)))
                .collect()
        };
        match phase {
            TouchPhase::Start => {
                let Some(landed) = test.hit(at) else {
                    return;
                };
                let earlier = fingers(self);
                let target = match earlier.first() {
                    None => landed,
                    // A finger landing while others are held joins them in one gesture: it goes
                    // to the innermost surface placing all of them, or further out where that
                    // surface grants its placer the gesture here.
                    Some(_) => {
                        let mut shared = landed;
                        for press in &earlier {
                            if let Some(held) = press.target {
                                shared = common(test, shared, held).unwrap_or(shared);
                            }
                        }
                        raised(test, shared, at)
                    }
                };
                for press in earlier.iter().filter(|press| press.target != Some(target)) {
                    if let (Pointer::Touch(id), Some(held)) = (press.pointer, press.target) {
                        let cancel = Input::Touch {
                            id,
                            phase: TouchPhase::Cancel,
                            pos: press.at,
                            force: None,
                        };
                        deliver(test, held, cancel, press.at, out);
                    }
                }
                for press in &mut self.presses {
                    if let Pointer::Touch(id) = press.pointer
                        && press.target != Some(target)
                    {
                        press.target = Some(target);
                        let start = Input::Touch {
                            id,
                            phase: TouchPhase::Start,
                            pos: press.at,
                            force: None,
                        };
                        deliver(test, target, start, press.at, out);
                    }
                }
                if earlier.is_empty() {
                    self.press_focus(test, target, out);
                }
                self.presses.retain(|press| press.pointer != finger);
                self.presses.push(Press {
                    pointer: finger,
                    button: PointerButton::Primary,
                    target: Some(target),
                    at,
                });
                deliver(test, target, input, at, out);
            }
            TouchPhase::Move => {
                if self.pressing(finger)
                    && let Some(target) = self.moved(test, finger, at)
                {
                    deliver(test, target, input, at, out);
                }
            }
            TouchPhase::End | TouchPhase::Cancel => {
                if self.pressing(finger)
                    && let Some(target) = self.cancel(finger)
                {
                    deliver(test, target, input, at, out);
                }
            }
        }
    }

    /// Tells the surfaces the pointer left, now that `under` is under it.
    fn left_into(&mut self, under: Option<T>, out: &mut Vec<Routed<T>>) {
        for target in self.hover(under) {
            out.push(Routed::Input {
                target,
                input: Input::PointerLeft,
            });
        }
    }

    fn press_focus<H: HitTest<Target = T>>(
        &mut self,
        test: &H,
        target: T,
        out: &mut Vec<Routed<T>>,
    ) {
        if test.takes_focus(target) {
            self.move_focus(Some(target), out);
        }
    }

    fn move_focus(&mut self, target: Option<T>, out: &mut Vec<Routed<T>>) {
        if self.focused == target {
            return;
        }
        if self.locked.is_some() && self.locked != target {
            self.unlock_into(out);
        }
        if let Some(had) = std::mem::replace(&mut self.focused, target) {
            out.push(Routed::Input {
                target: had,
                input: Input::Focus(false),
            });
        }
        if let Some(target) = target {
            out.push(Routed::Input {
                target,
                input: Input::Focus(true),
            });
        }
    }

    fn unlock_into(&mut self, out: &mut Vec<Routed<T>>) {
        if let Some(target) = self.locked.take() {
            out.push(Routed::Input {
                target,
                input: Input::PointerUnlocked,
            });
        }
    }

    fn to_focus(&self, input: Input, out: &mut Vec<Routed<T>>) {
        if let Some(target) = self.focused {
            out.push(Routed::Input { target, input });
        }
    }

    fn carry_over<H: HitTest<Target = T>>(&mut self, test: &H, at: Pos2, out: &mut Vec<Routed<T>>) {
        let Some(carry) = &mut self.carry else {
            return;
        };
        let under = test.hit(at);
        if carry.over != under
            && let Some(left) = std::mem::replace(&mut carry.over, under)
        {
            out.push(Routed::Drag {
                target: left,
                drag: Drag::Left,
            });
        }
        if let Some(target) = under
            && let Some(local) = test.local(target, at)
        {
            out.push(Routed::Drag {
                target,
                drag: Drag::Over(local),
            });
        }
    }

    fn end_carry(&mut self, out: &mut Vec<Routed<T>>) {
        if let Some(Carry {
            over: Some(over), ..
        }) = self.carry.take()
        {
            out.push(Routed::Drag {
                target: over,
                drag: Drag::Left,
            });
        }
    }
}

/// Delivers `input` to `target` with its position (`at`, in the window) in `target`'s own
/// coordinates; nothing where `target` is gone.
fn deliver<H: HitTest>(
    test: &H,
    target: H::Target,
    input: Input,
    at: Pos2,
    out: &mut Vec<Routed<H::Target>>,
) {
    let Some(local) = test.local(target, at) else {
        return;
    };
    out.push(Routed::Input {
        target,
        input: placed(input, local),
    });
}

/// `input` with its position moved to `at`.
fn placed(mut input: Input, at: Pos2) -> Input {
    match &mut input {
        Input::PointerMoved(pos)
        | Input::PointerRested(pos)
        | Input::Predicted(pos)
        | Input::PointerButton { pos, .. }
        | Input::Touch { pos, .. }
        | Input::Pen { pos, .. } => *pos = at,
        _ => {}
    }
    input
}

/// Who takes a wheel, pinch or pan-drag at `at`: the surface under it, or its outermost placer
/// that the surfaces between grant the gesture there.
fn claimant<H: HitTest>(test: &H, at: Pos2) -> Option<H::Target> {
    Some(raised(test, test.hit(at)?, at))
}

/// `target`, or the outermost of its placers whose direct child in the chain grants it the
/// gesture at `at` (its gesture area): gesture claims go outermost first
/// (guides/pan_and_zoom.md, Gestures), so that placer wins whether or not the surfaces between
/// grant theirs.
fn raised<H: HitTest>(test: &H, target: H::Target, at: Pos2) -> H::Target {
    let mut chain = vec![target];
    while let Some(placer) = test.placer(*chain.last().unwrap_or(&target)) {
        if chain.contains(&placer) {
            break;
        }
        chain.push(placer);
    }
    for index in (1..chain.len()).rev() {
        if test.grants_gestures(chain[index - 1], at) {
            return chain[index];
        }
    }
    target
}

/// The innermost surface that is, or places, both `one` and `other`.
fn common<H: HitTest>(test: &H, one: H::Target, other: H::Target) -> Option<H::Target> {
    let mut placers = vec![one];
    while let Some(placer) = test.placer(*placers.last().unwrap_or(&one)) {
        if placers.contains(&placer) {
            break;
        }
        placers.push(placer);
    }
    let mut at = Some(other);
    let mut seen = Vec::new();
    while let Some(target) = at {
        if placers.contains(&target) {
            return Some(target);
        }
        if seen.contains(&target) {
            break;
        }
        seen.push(target);
        at = test.placer(target);
    }
    None
}

#[cfg(test)]
mod tests;
