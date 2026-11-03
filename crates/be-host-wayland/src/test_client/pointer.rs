use wayland_client::protocol::{wl_pointer, wl_region, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_confined_pointer_v1, zwp_locked_pointer_v1, zwp_pointer_constraints_v1,
};
use wayland_protocols::wp::pointer_gestures::zv1::client::{
    zwp_pointer_gesture_hold_v1, zwp_pointer_gesture_pinch_v1, zwp_pointer_gesture_swipe_v1,
    zwp_pointer_gestures_v1,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1, zwp_relative_pointer_v1,
};

use super::{Received, TestClient};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PointerSeen {
    Locked,
    Unlocked,
    Confined,
    Unconfined,
    Relative(f64, f64, f64, f64),
    SwipeBegan(u32),
    Swiped(f64, f64),
    SwipeEnded(bool),
    PinchBegan(u32),
    Pinched(f64, f64, f64, f64),
    PinchEnded(bool),
    HoldBegan(u32),
    HoldEnded(bool),
}

pub(crate) type Gestures = (
    zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
    zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
    zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1,
);

impl TestClient {
    fn constraints(&self) -> zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 {
        self.bind("zwp_pointer_constraints_v1", 1)
    }

    pub(crate) fn region(&self, rect: (i32, i32, i32, i32)) -> wl_region::WlRegion {
        let region = self
            .compositor
            .as_ref()
            .unwrap()
            .create_region(&self.handle, ());
        region.add(rect.0, rect.1, rect.2, rect.3);
        region
    }

    pub(crate) fn lock_pointer(
        &self,
        surface: &wl_surface::WlSurface,
        pointer: &wl_pointer::WlPointer,
    ) -> zwp_locked_pointer_v1::ZwpLockedPointerV1 {
        self.constraints().lock_pointer(
            surface,
            pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Persistent,
            &self.handle,
            (),
        )
    }

    pub(crate) fn confine_pointer(
        &self,
        surface: &wl_surface::WlSurface,
        pointer: &wl_pointer::WlPointer,
        region: &wl_region::WlRegion,
    ) -> zwp_confined_pointer_v1::ZwpConfinedPointerV1 {
        self.constraints().confine_pointer(
            surface,
            pointer,
            Some(region),
            zwp_pointer_constraints_v1::Lifetime::Persistent,
            &self.handle,
            (),
        )
    }

    pub(crate) fn relative_pointer(
        &self,
        pointer: &wl_pointer::WlPointer,
    ) -> zwp_relative_pointer_v1::ZwpRelativePointerV1 {
        let manager: zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1 =
            self.bind("zwp_relative_pointer_manager_v1", 1);
        manager.get_relative_pointer(pointer, &self.handle, ())
    }

    pub(crate) fn gestures(&self, pointer: &wl_pointer::WlPointer) -> Gestures {
        let manager: zwp_pointer_gestures_v1::ZwpPointerGesturesV1 =
            self.bind("zwp_pointer_gestures_v1", 3);
        (
            manager.get_swipe_gesture(pointer, &self.handle, ()),
            manager.get_pinch_gesture(pointer, &self.handle, ()),
            manager.get_hold_gesture(pointer, &self.handle, ()),
        )
    }
}

wayland_client::delegate_noop!(Received: ignore wl_region::WlRegion);
wayland_client::delegate_noop!(Received: ignore zwp_pointer_constraints_v1::ZwpPointerConstraintsV1);
wayland_client::delegate_noop!(Received: ignore zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1);
wayland_client::delegate_noop!(Received: ignore zwp_pointer_gestures_v1::ZwpPointerGesturesV1);

impl Dispatch<zwp_locked_pointer_v1::ZwpLockedPointerV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_locked_pointer_v1::ZwpLockedPointerV1,
        event: zwp_locked_pointer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.protocols.pointer.push(match event {
            zwp_locked_pointer_v1::Event::Locked => PointerSeen::Locked,
            zwp_locked_pointer_v1::Event::Unlocked => PointerSeen::Unlocked,
            _ => return,
        });
    }
}

impl Dispatch<zwp_confined_pointer_v1::ZwpConfinedPointerV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_confined_pointer_v1::ZwpConfinedPointerV1,
        event: zwp_confined_pointer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.protocols.pointer.push(match event {
            zwp_confined_pointer_v1::Event::Confined => PointerSeen::Confined,
            zwp_confined_pointer_v1::Event::Unconfined => PointerSeen::Unconfined,
            _ => return,
        });
    }
}

impl Dispatch<zwp_relative_pointer_v1::ZwpRelativePointerV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_relative_pointer_v1::ZwpRelativePointerV1,
        event: zwp_relative_pointer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_relative_pointer_v1::Event::RelativeMotion {
            dx,
            dy,
            dx_unaccel,
            dy_unaccel,
            ..
        } = event
        {
            let seen = PointerSeen::Relative(dx, dy, dx_unaccel, dy_unaccel);
            state.protocols.pointer.push(seen);
        }
    }
}

impl Dispatch<zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
        event: zwp_pointer_gesture_swipe_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.protocols.pointer.push(match event {
            zwp_pointer_gesture_swipe_v1::Event::Begin { fingers, .. } => {
                PointerSeen::SwipeBegan(fingers)
            }
            zwp_pointer_gesture_swipe_v1::Event::Update { dx, dy, .. } => {
                PointerSeen::Swiped(dx, dy)
            }
            zwp_pointer_gesture_swipe_v1::Event::End { cancelled, .. } => {
                PointerSeen::SwipeEnded(cancelled != 0)
            }
            _ => return,
        });
    }
}

impl Dispatch<zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
        event: zwp_pointer_gesture_pinch_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.protocols.pointer.push(match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin { fingers, .. } => {
                PointerSeen::PinchBegan(fingers)
            }
            zwp_pointer_gesture_pinch_v1::Event::Update {
                dx,
                dy,
                scale,
                rotation,
                ..
            } => PointerSeen::Pinched(dx, dy, scale, rotation),
            zwp_pointer_gesture_pinch_v1::Event::End { cancelled, .. } => {
                PointerSeen::PinchEnded(cancelled != 0)
            }
            _ => return,
        });
    }
}

impl Dispatch<zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1,
        event: zwp_pointer_gesture_hold_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.protocols.pointer.push(match event {
            zwp_pointer_gesture_hold_v1::Event::Begin { fingers, .. } => {
                PointerSeen::HoldBegan(fingers)
            }
            zwp_pointer_gesture_hold_v1::Event::End { cancelled, .. } => {
                PointerSeen::HoldEnded(cancelled != 0)
            }
            _ => return,
        });
    }
}
