use beui::{TouchpadGesture, Vec2};
use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
    GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
    MotionEvent, PointerHandle, RelativeMotionEvent,
};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, SERIAL_COUNTER};
use smithay::wayland::pointer_constraints::{
    PointerConstraint, PointerConstraintsHandler, PointerConstraintsState, with_pointer_constraint,
};
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::{delegate_pointer_constraints, delegate_pointer_gestures, delegate_relative_pointer};

use crate::state::{State, SurfaceAt, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldPointer {
    Locked,
    Confined,
}

#[derive(Default)]
pub(crate) struct Constraints {
    held: Option<Held>,
    released: Option<WlSurface>,
}

#[derive(Clone)]
struct Held {
    surface: WlSurface,
    window: WindowId,
    kind: HeldPointer,
}

pub(crate) fn create(handle: &DisplayHandle) {
    PointerConstraintsState::new::<State>(handle);
    RelativePointerManagerState::new::<State>(handle);
    PointerGesturesState::new::<State>(handle);
}

fn kind_of(constraint: &PointerConstraint) -> HeldPointer {
    match constraint {
        PointerConstraint::Locked(_) => HeldPointer::Locked,
        PointerConstraint::Confined(_) => HeldPointer::Confined,
    }
}

impl State {
    pub(crate) fn held_pointer(&self, id: WindowId) -> Option<HeldPointer> {
        self.constraints
            .held
            .as_ref()
            .filter(|held| held.window == id)
            .map(|held| held.kind)
    }

    fn surface_under_pointer(&self) -> Option<SurfaceAt> {
        self.under(self.pointer_target)?.2
    }

    fn inside(
        &self,
        surface: &WlSurface,
        pointer: &PointerHandle<Self>,
        at: Point<f64, Logical>,
    ) -> bool {
        with_pointer_constraint(surface, pointer, |constraint| {
            constraint.is_some_and(|constraint| {
                constraint
                    .region()
                    .is_none_or(|region| region.contains(at.to_i32_floor()))
            })
        })
    }

    fn wanted(&self, pointer: &PointerHandle<Self>) -> Option<Held> {
        let (surface, origin) = self.surface_under_pointer()?;
        if pointer.current_focus().as_ref() != Some(&surface)
            || self.constraints.released.as_ref() == Some(&surface)
        {
            return None;
        }
        let window = self.window_of_surface(&surface)?;
        if self.keyboard_window != Some(window) {
            return None;
        }
        let at = pointer.current_location() - origin;
        let kind = with_pointer_constraint(&surface, pointer, |constraint| {
            constraint.map(|constraint| (kind_of(&constraint), constraint.is_active()))
        });
        let (kind, active) = kind?;
        (active || self.inside(&surface, pointer, at)).then_some(Held {
            surface,
            window,
            kind,
        })
    }

    pub fn settle_constraints(&mut self) -> bool {
        let pointer = self.pointer();
        if let Some(released) = &self.constraints.released
            && self.window_of_surface(released) != self.keyboard_window
        {
            self.constraints.released = None;
        }
        let wanted = self.wanted(&pointer);
        let held = self.constraints.held.as_ref();
        let same =
            |held: &Held, wanted: &Held| held.surface == wanted.surface && held.kind == wanted.kind;
        if let (Some(held), Some(wanted)) = (held, &wanted)
            && same(held, wanted)
        {
            return false;
        }
        if held.is_none() && wanted.is_none() {
            return false;
        }
        self.let_go();
        if let Some(wanted) = wanted {
            with_pointer_constraint(&wanted.surface, &pointer, |constraint| {
                if let Some(constraint) = constraint.filter(|constraint| !constraint.is_active()) {
                    constraint.activate();
                }
            });
            self.changed(wanted.window);
            self.constraints.held = Some(wanted);
        }
        true
    }

    fn let_go(&mut self) {
        let Some(held) = self.constraints.held.take() else {
            return;
        };
        self.changed(held.window);
        if !held.surface.alive() {
            return;
        }
        let pointer = self.pointer();
        let hint = with_pointer_constraint(&held.surface, &pointer, |constraint| {
            let constraint = constraint?;
            let hint = match &*constraint {
                PointerConstraint::Locked(locked) => locked.cursor_position_hint(),
                PointerConstraint::Confined(_) => None,
            };
            if constraint.is_active() {
                constraint.deactivate();
            }
            hint
        });
        let Some(hint) = hint else {
            return;
        };
        let Some((surface, origin)) = self
            .surface_under_pointer()
            .filter(|(surface, _)| *surface == held.surface)
        else {
            return;
        };
        if pointer.current_focus().as_ref() != Some(&surface) {
            return;
        }
        let time = self.time();
        pointer.motion(
            self,
            Some((surface, origin)),
            &MotionEvent {
                location: origin + hint,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
        pointer.frame(self);
    }

    pub fn release_pointer(&mut self, id: WindowId) {
        let Some(held) = self
            .constraints
            .held
            .clone()
            .filter(|held| held.window == id)
        else {
            return;
        };
        self.let_go();
        self.constraints.released = Some(held.surface);
    }

    pub(crate) fn release_all_pointers(&mut self) {
        self.let_go();
    }

    pub(crate) fn pressed_on(&mut self, target: Option<WindowId>) {
        if let Some(released) = &self.constraints.released
            && target.is_some()
            && self.window_of_surface(released) == target
        {
            self.constraints.released = None;
        }
        if let Some(held) = &self.constraints.held
            && Some(held.window) != target
        {
            let window = held.window;
            self.release_pointer(window);
        }
    }

    pub(crate) fn holds_pointer_from(
        &self,
        target: Option<(WindowId, Point<f64, Logical>)>,
    ) -> bool {
        let Some(held) = &self.constraints.held else {
            return false;
        };
        if held.kind == HeldPointer::Locked {
            return true;
        }
        let Some((id, location, Some((surface, origin)))) = self.under(target) else {
            return true;
        };
        id != held.window
            || surface != held.surface
            || !self.inside(&surface, &self.pointer(), location - origin)
    }

    pub(crate) fn moved_by(
        &mut self,
        delta: Point<f64, Logical>,
        unaccelerated: Point<f64, Logical>,
    ) {
        let pointer = self.pointer();
        let Some(focus) = pointer
            .current_focus()
            .and_then(|focus| self.surface_under_pointer().filter(|(at, _)| *at == focus))
        else {
            return;
        };
        let utime = self.start.elapsed().as_micros() as u64;
        pointer.relative_motion(
            self,
            Some(focus),
            &RelativeMotionEvent {
                delta,
                delta_unaccel: unaccelerated,
                utime,
            },
        );
    }

    pub fn pointer_relative(&mut self, delta: Vec2) {
        if self.held_pointer_kind() != Some(HeldPointer::Locked) {
            return;
        }
        let delta = (f64::from(delta.x), f64::from(delta.y)).into();
        self.moved_by(delta, delta);
        self.pointer().frame(self);
    }

    fn held_pointer_kind(&self) -> Option<HeldPointer> {
        self.constraints.held.as_ref().map(|held| held.kind)
    }

    pub fn touchpad(&mut self, gesture: TouchpadGesture) {
        let pointer = self.pointer();
        let time = self.time();
        let serial = SERIAL_COUNTER.next_serial();
        let point = |delta: Vec2| -> Point<f64, Logical> {
            (f64::from(delta.x), f64::from(delta.y)).into()
        };
        match gesture {
            TouchpadGesture::SwipeBegan(fingers) => pointer.gesture_swipe_begin(
                self,
                &GestureSwipeBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            TouchpadGesture::Swiped(delta) => pointer.gesture_swipe_update(
                self,
                &GestureSwipeUpdateEvent {
                    time,
                    delta: point(delta),
                },
            ),
            TouchpadGesture::SwipeEnded(cancelled) => pointer.gesture_swipe_end(
                self,
                &GestureSwipeEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
            TouchpadGesture::PinchBegan(fingers) => pointer.gesture_pinch_begin(
                self,
                &GesturePinchBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            TouchpadGesture::Pinched {
                delta,
                scale,
                rotation,
            } => pointer.gesture_pinch_update(
                self,
                &GesturePinchUpdateEvent {
                    time,
                    delta: point(delta),
                    scale: f64::from(scale),
                    rotation: f64::from(rotation),
                },
            ),
            TouchpadGesture::PinchEnded(cancelled) => pointer.gesture_pinch_end(
                self,
                &GesturePinchEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
            TouchpadGesture::HoldBegan(fingers) => pointer.gesture_hold_begin(
                self,
                &GestureHoldBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            TouchpadGesture::HoldEnded(cancelled) => pointer.gesture_hold_end(
                self,
                &GestureHoldEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
        }
    }
}

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, _surface: &WlSurface, _pointer: &PointerHandle<Self>) {
        self.settle_constraints();
    }

    fn cursor_position_hint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        _location: Point<f64, Logical>,
    ) {
    }
}

delegate_pointer_constraints!(State);
delegate_relative_pointer!(State);
delegate_pointer_gestures!(State);
