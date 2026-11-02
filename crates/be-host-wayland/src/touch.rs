use std::collections::HashMap;

use beui::TouchId;
use smithay::backend::input::TouchSlot;
use smithay::input::SeatHandler;
use smithay::input::touch::{
    DownEvent, GrabStartData, MotionEvent, OrientationEvent, ShapeEvent, TouchGrab, TouchHandle,
    TouchInnerHandle, UpEvent,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_seat::{self, WlSeat};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::protocol::wl_touch::{self, WlTouch};
use smithay::reexports::wayland_server::{Client, DataInit, Dispatch, DisplayHandle, Resource};
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Serial};
use smithay::wayland::seat::{SeatUserData, TouchUserData};

use crate::state::{State, WindowId};

const BTN_LEFT: u32 = 0x110;

#[derive(Default)]
pub(crate) struct Touch {
    clients: HashMap<ClientId, usize>,
    points: Vec<Held>,
    emulated: Option<TouchId>,
    seen: bool,
}

struct Held {
    id: TouchId,
    slot: u32,
    window: Option<WindowId>,
}

impl Touch {
    fn takes_touch(&self, surface: &WlSurface) -> bool {
        surface.client().is_some_and(|client| {
            self.clients
                .get(&client.id())
                .is_some_and(|count| *count > 0)
        })
    }

    fn free_slot(&self) -> u32 {
        (0..)
            .find(|slot| self.points.iter().all(|held| held.slot != *slot))
            .unwrap_or(0)
    }

    fn slot(&self, id: TouchId) -> Option<TouchSlot> {
        self.points
            .iter()
            .find(|held| held.id == id)
            .map(|held| TouchSlot::from(Some(held.slot)))
    }
}

impl State {
    pub fn set_touchscreen(&mut self, attached: bool) {
        let attached = attached || self.touch.seen;
        if attached == self.seat.get_touch().is_some() {
            return;
        }
        self.touch_cancel_all();
        self.touch.clients.clear();
        match attached {
            true => {
                self.seat
                    .add_touch_with_default_grab(|| Box::new(EachPointOnItsOwn));
            }
            false => self.seat.remove_touch(),
        }
    }

    pub fn touch_seen(&mut self) {
        self.touch.seen = true;
    }

    pub fn touching(&self, id: WindowId) -> bool {
        self.touch.points.iter().any(|held| held.window == Some(id))
            || (self.touch.emulated.is_some() && self.pointer_window() == Some(id))
    }

    pub fn touch_down(&mut self, id: TouchId, target: Option<(WindowId, Point<f64, Logical>)>) {
        let focus = self.under(target);
        let surface = focus
            .as_ref()
            .and_then(|(_, _, under)| under.as_ref())
            .map(|(surface, _)| surface.clone());
        let touch = self.seat.get_touch();
        match (&surface, touch) {
            (Some(surface), Some(touch)) if self.touch.takes_touch(surface) => {
                let Some((window, location, under)) = focus else {
                    return;
                };
                let slot = self.touch.free_slot();
                self.touch.points.push(Held {
                    id,
                    slot,
                    window: Some(window),
                });
                let event = DownEvent {
                    slot: TouchSlot::from(Some(slot)),
                    location,
                    serial: SERIAL_COUNTER.next_serial(),
                    time: self.time(),
                };
                touch.down(self, under, &event);
            }
            (Some(_), _) if self.touch.emulated.is_none() => {
                self.touch.emulated = Some(id);
                self.pointer_motion(target);
                self.pointer_button(BTN_LEFT, true);
            }
            _ => {}
        }
    }

    pub fn touch_motion(&mut self, id: TouchId, target: Option<(WindowId, Point<f64, Logical>)>) {
        if self.touch.emulated == Some(id) {
            self.pointer_motion(target);
            return;
        }
        let (Some(slot), Some(touch)) = (self.touch.slot(id), self.seat.get_touch()) else {
            return;
        };
        let Some((_, location, _)) = self.under(target) else {
            return;
        };
        let event = MotionEvent {
            slot,
            location,
            time: self.time(),
        };
        touch.motion(self, None, &event);
    }

    pub fn touch_up(&mut self, id: TouchId) {
        if self.touch.emulated == Some(id) {
            self.touch.emulated = None;
            self.pointer_button(BTN_LEFT, false);
            return;
        }
        let Some(slot) = self.touch.slot(id) else {
            return;
        };
        self.touch.points.retain(|held| held.id != id);
        if let Some(touch) = self.seat.get_touch() {
            let event = UpEvent {
                slot,
                serial: SERIAL_COUNTER.next_serial(),
                time: self.time(),
            };
            touch.up(self, &event);
        }
    }

    pub fn touch_cancel(&mut self, id: TouchId) {
        if self.touch.emulated == Some(id) {
            self.touch_up(id);
            return;
        }
        if self.touch.slot(id).is_some() {
            self.touch_cancel_all();
        }
    }

    pub fn touch_frame(&mut self) {
        if let Some(touch) = self.seat.get_touch() {
            touch.frame(self);
        }
    }

    fn touch_cancel_all(&mut self) {
        if std::mem::take(&mut self.touch.points).is_empty() {
            return;
        }
        if let Some(touch) = self.seat.get_touch() {
            touch.cancel(self);
        }
    }

    fn current_touch(&self, touch: &WlTouch) -> bool {
        TouchHandle::<State>::from_resource(touch).is_some_and(|handle| {
            self.seat
                .get_touch()
                .is_some_and(|current| current == handle)
        })
    }
}

struct EachPointOnItsOwn;

impl<D: SeatHandler + 'static> TouchGrab<D> for EachPointOnItsOwn {
    fn down(
        &mut self,
        data: &mut D,
        handle: &mut TouchInnerHandle<'_, D>,
        focus: Option<(<D as SeatHandler>::TouchFocus, Point<f64, Logical>)>,
        event: &DownEvent,
        seq: Serial,
    ) {
        handle.down(data, focus, event, seq);
    }

    fn up(
        &mut self,
        data: &mut D,
        handle: &mut TouchInnerHandle<'_, D>,
        event: &UpEvent,
        seq: Serial,
    ) {
        handle.up(data, event, seq);
    }

    fn motion(
        &mut self,
        data: &mut D,
        handle: &mut TouchInnerHandle<'_, D>,
        focus: Option<(<D as SeatHandler>::TouchFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
        seq: Serial,
    ) {
        handle.motion(data, focus, event, seq);
    }

    fn frame(&mut self, data: &mut D, handle: &mut TouchInnerHandle<'_, D>, seq: Serial) {
        handle.frame(data, seq);
    }

    fn cancel(&mut self, data: &mut D, handle: &mut TouchInnerHandle<'_, D>, seq: Serial) {
        handle.cancel(data, seq);
    }

    fn shape(
        &mut self,
        data: &mut D,
        handle: &mut TouchInnerHandle<'_, D>,
        event: &ShapeEvent,
        seq: Serial,
    ) {
        handle.shape(data, event, seq);
    }

    fn orientation(
        &mut self,
        data: &mut D,
        handle: &mut TouchInnerHandle<'_, D>,
        event: &OrientationEvent,
        seq: Serial,
    ) {
        handle.orientation(data, event, seq);
    }

    fn start_data(&self) -> &GrabStartData<D> {
        unreachable!("the default grab is never set, so nothing asks how it started")
    }

    fn unset(&mut self, _data: &mut D) {}
}

impl Dispatch<WlSeat, SeatUserData<State>> for State {
    fn request(
        state: &mut Self,
        client: &Client,
        resource: &WlSeat,
        request: wl_seat::Request,
        data: &SeatUserData<State>,
        handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if matches!(request, wl_seat::Request::GetTouch { .. }) && state.seat.get_touch().is_some()
        {
            *state.touch.clients.entry(client.id()).or_default() += 1;
        }
        <smithay::input::SeatState<State> as Dispatch<WlSeat, SeatUserData<State>, Self>>::request(
            state, client, resource, request, data, handle, data_init,
        );
    }

    fn destroyed(
        state: &mut Self,
        client: ClientId,
        resource: &WlSeat,
        data: &SeatUserData<State>,
    ) {
        <smithay::input::SeatState<State> as Dispatch<WlSeat, SeatUserData<State>, Self>>::destroyed(
            state, client, resource, data,
        );
    }
}

impl Dispatch<WlTouch, TouchUserData<State>> for State {
    fn request(
        state: &mut Self,
        client: &Client,
        resource: &WlTouch,
        request: wl_touch::Request,
        data: &TouchUserData<State>,
        handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        <smithay::input::SeatState<State> as Dispatch<WlTouch, TouchUserData<State>, Self>>::request(
            state, client, resource, request, data, handle, data_init,
        );
    }

    fn destroyed(
        state: &mut Self,
        client: ClientId,
        resource: &WlTouch,
        data: &TouchUserData<State>,
    ) {
        if state.current_touch(resource)
            && let Some(count) = state.touch.clients.get_mut(&client)
        {
            *count = count.saturating_sub(1);
        }
        <smithay::input::SeatState<State> as Dispatch<WlTouch, TouchUserData<State>, Self>>::destroyed(
            state, client, resource, data,
        );
    }
}
