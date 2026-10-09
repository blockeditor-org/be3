use std::time::{Duration, Instant};

use smithay::delegate_idle_inhibit;
use smithay::reexports::wayland_protocols::ext::idle_notify::v1::server::ext_idle_notification_v1::{
    self, ExtIdleNotificationV1,
};
use smithay::reexports::wayland_protocols::ext::idle_notify::v1::server::ext_idle_notifier_v1::{
    self, ExtIdleNotifierV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::idle_inhibit::{IdleInhibitHandler, IdleInhibitManagerState};

use crate::state::State;

const NOTIFIER_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug)]
struct Countdown {
    timeout: Duration,
    inhibitable: bool,
    since: Option<Instant>,
    idle: bool,
}

impl Countdown {
    fn new(timeout: Duration, inhibitable: bool) -> Self {
        Self {
            timeout,
            inhibitable,
            since: None,
            idle: false,
        }
    }

    fn due(&self, inhibited: bool) -> Option<Instant> {
        if self.idle || (self.inhibitable && inhibited) {
            return None;
        }
        Some(self.since? + self.timeout)
    }
}

struct Notification {
    resource: ExtIdleNotificationV1,
    countdown: Countdown,
}

pub(crate) struct Idle {
    inhibitors: Vec<WlSurface>,
    inhibited: bool,
    woken: bool,
    blank: Option<Countdown>,
    lock: Option<Countdown>,
    notifications: Vec<Notification>,
}

impl Idle {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        IdleInhibitManagerState::new::<State>(handle);
        handle.create_global::<State, ExtIdleNotifierV1, _>(NOTIFIER_VERSION, ());
        Self {
            inhibitors: Vec::new(),
            inhibited: false,
            blank: None,
            lock: None,
            notifications: Vec::new(),
            woken: false,
        }
    }

    pub(crate) fn set_blank_after(&mut self, after: Option<Duration>) {
        if self.blank.map(|blank| blank.timeout) == after {
            return;
        }
        self.blank = after.map(|after| Countdown::new(after, true));
    }

    pub(crate) fn set_lock_after(&mut self, after: Option<Duration>) {
        if self.lock.map(|lock| lock.timeout) == after {
            return;
        }
        self.lock = after.map(|after| Countdown::new(after, true));
    }

    pub(crate) fn lock_due(&self) -> bool {
        self.lock.is_some_and(|lock| lock.idle)
    }

    pub(crate) fn woke(&mut self) {
        self.woken = true;
    }

    pub(crate) fn blanked(&self) -> bool {
        self.blank.is_some_and(|blank| blank.idle)
    }

    pub(crate) fn inhibitors(&mut self) -> &[WlSurface] {
        self.inhibitors.retain(Resource::is_alive);
        &self.inhibitors
    }

    pub(crate) fn tick(&mut self, now: Instant, active: bool, inhibited: bool) -> Option<Instant> {
        let active = active || std::mem::take(&mut self.woken);
        let changed = inhibited != self.inhibited;
        self.inhibited = inhibited;
        self.notifications
            .retain(|notification| notification.resource.is_alive());
        let countdowns = self
            .blank
            .iter_mut()
            .chain(self.lock.iter_mut())
            .map(|countdown| (countdown, None))
            .chain(
            self.notifications
                .iter_mut()
                .map(|notification| (&mut notification.countdown, Some(&notification.resource))),
        );
        let mut next: Option<Instant> = None;
        for (countdown, resource) in countdowns {
            let resume = active || (changed && countdown.inhibitable);
            if countdown.since.is_none() || resume {
                countdown.since = Some(now);
            }
            if resume && countdown.idle {
                countdown.idle = false;
                if let Some(resource) = resource {
                    resource.resumed();
                }
            }
            match countdown.due(inhibited) {
                Some(due) if due <= now => {
                    countdown.idle = true;
                    if let Some(resource) = resource {
                        resource.idled();
                    }
                }
                Some(due) => next = Some(next.map_or(due, |next| next.min(due))),
                None => {}
            }
        }
        next
    }
}

impl IdleInhibitHandler for State {
    fn inhibit(&mut self, surface: WlSurface) {
        self.idle.inhibitors.push(surface);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        if let Some(index) = self
            .idle
            .inhibitors
            .iter()
            .position(|held| *held == surface)
        {
            self.idle.inhibitors.remove(index);
        }
    }
}

delegate_idle_inhibit!(State);

impl GlobalDispatch<ExtIdleNotifierV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ExtIdleNotifierV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ExtIdleNotifierV1,
        request: ext_idle_notifier_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let (id, timeout, inhibitable) = match request {
            ext_idle_notifier_v1::Request::GetIdleNotification { id, timeout, .. } => {
                (id, timeout, true)
            }
            ext_idle_notifier_v1::Request::GetInputIdleNotification { id, timeout, .. } => {
                (id, timeout, false)
            }
            _ => return,
        };
        let resource = data_init.init(id, ());
        state.idle.notifications.push(Notification {
            resource,
            countdown: Countdown::new(Duration::from_millis(timeout.into()), inhibitable),
        });
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ExtIdleNotificationV1,
        _request: ext_idle_notification_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &ExtIdleNotificationV1,
        _data: &(),
    ) {
        state
            .idle
            .notifications
            .retain(|notification| notification.resource != *resource);
    }
}
