use std::time::Duration;

use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{Monotonic, Time};
use smithay::wayland::commit_timing::{CommitTimerBarrierStateUserData, CommitTimingManagerState};
use smithay::wayland::compositor::SurfaceData;
use smithay::wayland::fifo::{FifoBarrierCachedState, FifoManagerState};
use smithay::{delegate_commit_timing, delegate_fifo};

use crate::state::{State, WindowId, client_compositor};

pub(crate) fn create(handle: &DisplayHandle) {
    FifoManagerState::new::<State>(handle);
    CommitTimingManagerState::new::<State>(handle);
}

fn signal_fifo(states: &SurfaceData) -> bool {
    let barrier = states
        .cached_state
        .get::<FifoBarrierCachedState>()
        .current()
        .barrier
        .take();
    barrier.is_some_and(|barrier| {
        barrier.signal();
        true
    })
}

impl State {
    pub(crate) fn release_fifo(&mut self, id: WindowId) {
        let Some(window) = self.find(id) else {
            return;
        };
        let mut signalled = false;
        window.window.with_surfaces(|_, states| {
            signalled |= signal_fifo(states);
        });
        if signalled {
            let client = self.surface(id).and_then(|surface| surface.client());
            self.cleared(client);
        }
    }

    pub(crate) fn release_timed(&mut self, until: Duration) -> Option<Duration> {
        let until = Time::<Monotonic>::from(until);
        let mut released = Vec::new();
        let mut next: Option<Duration> = None;
        for window in &self.windows {
            let mut signalled = false;
            window.window.with_surfaces(|_, states| {
                let Some(barriers) = states.data_map.get::<CommitTimerBarrierStateUserData>()
                else {
                    return;
                };
                let mut barriers = barriers.lock().unwrap();
                signalled |= barriers.signal_until(until);
                if let Some(deadline) = barriers.next_deadline() {
                    let deadline = Duration::from(Time::<Monotonic>::from(deadline));
                    next = Some(next.map_or(deadline, |next| next.min(deadline)));
                }
            });
            if signalled {
                released.push(window.id);
            }
        }
        for id in released {
            let client = self.surface(id).and_then(|surface| surface.client());
            self.cleared(client);
        }
        next
    }

    fn cleared(&mut self, client: Option<Client>) {
        let Some(client) = client else {
            return;
        };
        let handle = self.handle.clone();
        client_compositor(&client).blocker_cleared(self, &handle);
    }
}

delegate_fifo!(State);
delegate_commit_timing!(State);
