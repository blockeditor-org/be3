//! The forwarding router (plans/host_as_platform_adapter.md, Input): the rules beui applies to
//! its forwarding catchers (guides/beui_keyboard.md, One input path), for surfaces rather than
//! nodes. Given a hit test, it routes a window's raw input to the surface each event is for, in
//! that surface's own coordinates: a press captures the pointer until every button lifts, hover
//! follows the surface under the pointer, wheel and pinch go to the outermost placer whose
//! surface grants it the gesture there, a finger landing for an outer claim takes the earlier
//! fingers with it, keys go to the focus except the chords the compositor intercepts, and the
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

#[derive(Clone, Copy, Debug)]
struct Finger<T> {
    id: TouchId,
    target: T,
    at: Pos2,
}

#[derive(Clone, Copy, Debug)]
struct Carry<T> {
    from: T,
    over: Option<T>,
}

/// The state of one window's routing: who holds the press, the hover, the focus, the lock, the
/// fingers, and a drag.
#[derive(Debug)]
pub struct Router<T> {
    compositor: T,
    intercepted: Vec<KeyChord>,
    modifiers: Modifiers,
    pointer: Option<Pos2>,
    hovered: Option<T>,
    pressed: Option<T>,
    buttons: Vec<PointerButton>,
    pen: Option<T>,
    gesture: Option<T>,
    focused: Option<T>,
    locked: Option<T>,
    fingers: Vec<Finger<T>>,
    keys: Vec<(Key, T)>,
    carry: Option<Carry<T>>,
}

impl<T: Copy + Eq + Debug> Router<T> {
    /// A router whose intercepted chords and system gestures (back) go to `compositor`.
    pub fn new(compositor: T) -> Self {
        Self {
            compositor,
            intercepted: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: None,
            hovered: None,
            pressed: None,
            buttons: Vec::new(),
            pen: None,
            gesture: None,
            focused: None,
            locked: None,
            fingers: Vec::new(),
            keys: Vec::new(),
            carry: None,
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
        self.hovered
    }

    /// The surface a press is held for, until every button lifts.
    pub fn pressed(&self) -> Option<T> {
        self.pressed
    }

    pub fn locked(&self) -> Option<T> {
        self.locked
    }

    /// Routes one event of the window's raw input.
    pub fn route<H: HitTest<Target = T>>(&mut self, test: &H, input: Input) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        match input {
            Input::Focus(focused) => {
                if !focused {
                    self.unlock_into(&mut out);
                }
                self.to_focus(Input::Focus(focused), &mut out);
            }
            Input::Modifiers(modifiers) => {
                self.modifiers = modifiers;
                self.to_focus(Input::Modifiers(modifiers), &mut out);
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
                        let target = match self.intercepted.iter().any(|chord| chord.matches(press))
                        {
                            true => Some(self.compositor),
                            false => self.focused,
                        };
                        if let Some(target) = target {
                            self.keys.retain(|(held, _)| *held != key);
                            self.keys.push((key, target));
                        }
                        target
                    }
                    (Some(key), false) => {
                        // A release follows its press, wherever the focus went since.
                        match self.keys.iter().position(|(held, _)| *held == key) {
                            Some(index) => Some(self.keys.remove(index).1),
                            None => self.focused,
                        }
                    }
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
                out.push(Routed::Input {
                    target: self.compositor,
                    input,
                });
            }
            Input::PointerMoved(at) | Input::PointerRested(at) | Input::Predicted(at) => {
                self.pointer = Some(at);
                if self.carry.is_some() {
                    self.carry_over(test, at, &mut out);
                    return out;
                }
                let under = test.hit(at);
                if self.pressed.is_none() {
                    self.hover(under, &mut out);
                }
                if let Some(target) = self.pressed.or(under) {
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
                if let Some(carry) = &mut self.carry
                    && let Some(over) = carry.over.take()
                {
                    out.push(Routed::Drag {
                        target: over,
                        drag: Drag::Left,
                    });
                }
                if self.pressed.is_none() {
                    self.hover(None, &mut out);
                }
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
                let mut cancelled = Vec::new();
                cancelled.extend(self.pressed.take());
                cancelled.extend(self.pen.take());
                cancelled.extend(self.fingers.drain(..).map(|finger| finger.target));
                self.buttons.clear();
                let mut told = Vec::new();
                for target in cancelled {
                    if !told.contains(&target) {
                        told.push(target);
                        out.push(Routed::Input {
                            target,
                            input: Input::PointersCancelled,
                        });
                    }
                }
                self.end_carry(&mut out);
            }
            Input::ScrollStarted => {
                self.gesture = self.pointer.and_then(|at| claimant(test, at));
                self.to_gesture(input, &mut out);
            }
            Input::ScrollEnded => {
                self.to_gesture(input, &mut out);
                self.gesture = None;
            }
            Input::Scroll(_) | Input::Zoom(_) => match self.gesture {
                Some(_) => self.to_gesture(input, &mut out),
                None => {
                    if let Some(target) = self.pointer.and_then(|at| claimant(test, at)) {
                        out.push(Routed::Input { target, input });
                    }
                }
            },
            Input::Touchpad(gesture) => {
                let begins = matches!(
                    gesture,
                    TouchpadGesture::SwipeBegan(_)
                        | TouchpadGesture::PinchBegan(_)
                        | TouchpadGesture::HoldBegan(_)
                );
                let ends = matches!(
                    gesture,
                    TouchpadGesture::SwipeEnded(_)
                        | TouchpadGesture::PinchEnded(_)
                        | TouchpadGesture::HoldEnded(_)
                );
                if begins || self.gesture.is_none() {
                    self.gesture = self.pointer.and_then(|at| claimant(test, at));
                }
                self.to_gesture(Input::Touchpad(gesture), &mut out);
                if ends {
                    self.gesture = None;
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
                    PenPhase::Down(_) => {
                        let target = test.hit(pos);
                        if let Some(target) = target {
                            self.pen = Some(target);
                            self.press_focus(test, target, &mut out);
                        }
                        target
                    }
                    PenPhase::Up(_) | PenPhase::Gone => self.pen.take().or_else(|| test.hit(pos)),
                    PenPhase::Move => self.pen.or_else(|| test.hit(pos)),
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
        let holds = self.pressed == Some(from)
            || self.pen == Some(from)
            || self.fingers.iter().any(|finger| finger.target == from);
        if !holds {
            return out;
        }
        out.push(Routed::Input {
            target: from,
            input: Input::PointersCancelled,
        });
        if self.pressed == Some(from) {
            self.pressed = Some(placer);
            if let Some(at) = self.pointer {
                for button in self.buttons.clone() {
                    let input = Input::PointerButton {
                        pos: at,
                        button,
                        pressed: true,
                    };
                    deliver(test, placer, input, at, &mut out);
                }
            }
        }
        if self.pen == Some(from) {
            self.pen = Some(placer);
        }
        for finger in &mut self.fingers {
            if finger.target == from {
                finger.target = placer;
                let input = Input::Touch {
                    id: finger.id,
                    phase: TouchPhase::Start,
                    pos: finger.at,
                    force: None,
                };
                deliver(test, placer, input, finger.at, &mut out);
            }
        }
        out
    }

    /// `from`, holding a press, starts carrying something between surfaces: its press ends
    /// cancelled, and until every button lifts the surface under the pointer hears the carry
    /// over it, and the one under the release hears the drop.
    pub fn start_drag<H: HitTest<Target = T>>(&mut self, test: &H, from: T) -> Vec<Routed<T>> {
        let mut out = Vec::new();
        if self.pressed != Some(from) {
            return out;
        }
        out.push(Routed::Input {
            target: from,
            input: Input::PointersCancelled,
        });
        self.pressed = None;
        self.carry = Some(Carry { from, over: None });
        if let Some(at) = self.pointer {
            self.carry_over(test, at, &mut out);
        }
        out
    }

    /// Forgets a surface that went: it holds nothing any more, and nothing is sent to it.
    pub fn forget(&mut self, target: T) {
        let gone = |held: &mut Option<T>| {
            if *held == Some(target) {
                *held = None;
            }
        };
        gone(&mut self.hovered);
        gone(&mut self.pressed);
        gone(&mut self.pen);
        gone(&mut self.gesture);
        gone(&mut self.focused);
        gone(&mut self.locked);
        self.fingers.retain(|finger| finger.target != target);
        self.keys.retain(|(_, held)| *held != target);
        if let Some(carry) = &mut self.carry {
            if carry.from == target {
                self.carry = None;
            } else if carry.over == Some(target) {
                carry.over = None;
            }
        }
        if self.pressed.is_none() && self.carry.is_none() {
            self.buttons.clear();
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
            if !self.buttons.contains(&button) {
                self.buttons.push(button);
            }
            if self.carry.is_some() {
                return;
            }
            let target = match self.pressed {
                Some(held) => Some(held),
                None => {
                    let target = test.hit(at);
                    self.hover(target, out);
                    if let Some(target) = target {
                        self.pressed = Some(target);
                        self.press_focus(test, target, out);
                    }
                    target
                }
            };
            if let Some(target) = target {
                deliver(test, target, input, at, out);
            }
            return;
        }
        self.buttons.retain(|held| *held != button);
        let released = self.buttons.is_empty();
        if let Some(carry) = self.carry {
            if released {
                if let Some(over) = carry.over
                    && Some(over) != test.hit(at)
                {
                    out.push(Routed::Drag {
                        target: over,
                        drag: Drag::Left,
                    });
                }
                if let Some(target) = test.hit(at)
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
                self.hover(test.hit(at), out);
            }
            return;
        }
        let target = self.pressed.or_else(|| test.hit(at));
        if released {
            self.pressed = None;
        }
        if let Some(target) = target {
            deliver(test, target, input, at, out);
        }
        if released {
            self.hover(test.hit(at), out);
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
        match phase {
            TouchPhase::Start => {
                let Some(landed) = test.hit(at) else {
                    return;
                };
                let target = match self.fingers.first() {
                    None => landed,
                    // A finger landing while others are held joins them in one gesture: it goes
                    // to the innermost surface placing all of them, or further out where that
                    // surface grants its placer the gesture here.
                    Some(_) => {
                        let mut shared = landed;
                        for finger in &self.fingers {
                            shared = common(test, shared, finger.target).unwrap_or(shared);
                        }
                        raised(test, shared, at)
                    }
                };
                let earlier: Vec<Finger<T>> = self
                    .fingers
                    .iter()
                    .copied()
                    .filter(|finger| finger.target != target)
                    .collect();
                let mut cancelled = Vec::new();
                for finger in &earlier {
                    let cancel = Input::Touch {
                        id: finger.id,
                        phase: TouchPhase::Cancel,
                        pos: finger.at,
                        force: None,
                    };
                    deliver(test, finger.target, cancel, finger.at, out);
                    if !cancelled.contains(&finger.target) {
                        cancelled.push(finger.target);
                    }
                }
                for finger in &mut self.fingers {
                    if finger.target != target {
                        finger.target = target;
                        let start = Input::Touch {
                            id: finger.id,
                            phase: TouchPhase::Start,
                            pos: finger.at,
                            force: None,
                        };
                        deliver(test, target, start, finger.at, out);
                    }
                }
                if self.fingers.is_empty() {
                    self.press_focus(test, target, out);
                }
                self.fingers.retain(|finger| finger.id != id);
                self.fingers.push(Finger { id, target, at });
                deliver(test, target, input, at, out);
            }
            TouchPhase::Move => {
                if let Some(finger) = self.fingers.iter_mut().find(|finger| finger.id == id) {
                    finger.at = at;
                    let target = finger.target;
                    deliver(test, target, input, at, out);
                }
            }
            TouchPhase::End | TouchPhase::Cancel => {
                if let Some(index) = self.fingers.iter().position(|finger| finger.id == id) {
                    let finger = self.fingers.remove(index);
                    deliver(test, finger.target, input, at, out);
                }
            }
        }
    }

    fn hover(&mut self, under: Option<T>, out: &mut Vec<Routed<T>>) {
        if self.hovered == under {
            return;
        }
        if let Some(left) = std::mem::replace(&mut self.hovered, under) {
            out.push(Routed::Input {
                target: left,
                input: Input::PointerLeft,
            });
        }
    }

    fn press_focus<H: HitTest<Target = T>>(&mut self, test: &H, target: T, out: &mut Vec<Routed<T>>) {
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

    fn to_gesture(&self, input: Input, out: &mut Vec<Routed<T>>) {
        if let Some(target) = self.gesture {
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

/// `target`, or its outermost placer it grants the gesture at `at`: gesture claims go outermost
/// first (guides/pan_and_zoom.md, Gestures), so the outermost camera granted it wins.
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
