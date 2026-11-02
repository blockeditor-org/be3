use std::collections::HashMap;
use std::time::{Duration, Instant};

use beui::Rect;
use smithay::delegate_presentation;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback::Kind;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Clock, ClockSource, Monotonic};
use smithay::wayland::compositor::with_states;
use smithay::wayland::presentation::{
    PresentationFeedbackCachedState, PresentationFeedbackCallback, PresentationState, Refresh,
};

use crate::state::{State, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presented {
    pub time: Duration,
    pub refresh: Duration,
    pub sequence: u64,
    pub vsync: bool,
    pub hardware_clock: bool,
}

impl Presented {
    fn kind(&self) -> Kind {
        let mut kind = Kind::empty();
        if self.vsync {
            kind |= Kind::Vsync | Kind::HwCompletion;
        }
        if self.hardware_clock {
            kind |= Kind::HwClock;
        }
        kind
    }
}

type Callbacks = Vec<PresentationFeedbackCallback>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Place {
    Anywhere,
    On(Rect),
    Nowhere,
}

pub(crate) struct Shown {
    pub(crate) id: WindowId,
    callbacks: Callbacks,
    pub(crate) presented: Presented,
}

pub(crate) struct Presentation {
    _global: PresentationState,
    epoch: (Instant, Duration),
    paced: bool,
    drawn: HashMap<WindowId, Callbacks>,
    painted: Vec<(WindowId, Callbacks)>,
    flips: Vec<(Rect, Presented)>,
}

fn discard(callbacks: Callbacks) {
    for callback in callbacks {
        callback.discarded();
    }
}

impl Presentation {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        Self {
            _global: PresentationState::new::<State>(handle, Monotonic::ID as u32),
            epoch: (Instant::now(), Clock::<Monotonic>::new().now().into()),
            paced: false,
            drawn: HashMap::new(),
            painted: Vec::new(),
            flips: Vec::new(),
        }
    }

    pub(crate) fn pace_to_flips(&mut self) {
        self.paced = true;
    }

    pub(crate) fn flipped(&mut self, screen: Rect, presented: Presented) {
        self.flips.retain(|(held, _)| *held != screen);
        self.flips.push((screen, presented));
    }

    pub(crate) fn awaits_flip(&self) -> bool {
        self.paced && !self.painted.is_empty()
    }

    pub(crate) fn monotonic(&self, now: Instant) -> Duration {
        let (instant, monotonic) = self.epoch;
        monotonic + now.saturating_duration_since(instant)
    }

    pub(crate) fn drawn(&mut self, id: WindowId, callbacks: Callbacks) {
        if let Some(superseded) = self.drawn.insert(id, callbacks) {
            discard(superseded);
        }
    }

    pub(crate) fn painted(&mut self, id: WindowId) {
        let callbacks = self.drawn.remove(&id).unwrap_or_default();
        match self.painted.iter_mut().find(|(held, _)| *held == id) {
            Some((_, held)) => {
                if !callbacks.is_empty() {
                    discard(std::mem::replace(held, callbacks));
                }
            }
            None => self.painted.push((id, callbacks)),
        }
    }

    pub(crate) fn forget_closed(&mut self, open: &[WindowId]) {
        let closed: Vec<_> = self
            .drawn
            .keys()
            .filter(|id| !open.contains(id))
            .copied()
            .collect();
        for id in closed {
            if let Some(callbacks) = self.drawn.remove(&id) {
                discard(callbacks);
            }
        }
        let (kept, closed) = std::mem::take(&mut self.painted)
            .into_iter()
            .partition(|(id, _)| open.contains(id));
        self.painted = kept;
        for (_, callbacks) in closed {
            discard(callbacks);
        }
    }

    pub(crate) fn take_shown(
        &mut self,
        now: Instant,
        place_of: impl Fn(WindowId) -> Place,
    ) -> Vec<Shown> {
        let flips = std::mem::take(&mut self.flips);
        if !self.paced {
            let presented = Presented {
                time: self.monotonic(now),
                refresh: Duration::ZERO,
                sequence: 0,
                vsync: false,
                hardware_clock: false,
            };
            return std::mem::take(&mut self.painted)
                .into_iter()
                .map(|(id, callbacks)| Shown {
                    id,
                    callbacks,
                    presented,
                })
                .collect();
        }
        let mut shown = Vec::new();
        let mut waiting = Vec::new();
        for (id, mut callbacks) in std::mem::take(&mut self.painted) {
            let place = place_of(id);
            if place == Place::Nowhere {
                discard(std::mem::take(&mut callbacks));
            }
            let flip = flips.iter().rev().find(|(flipped, _)| match place {
                Place::On(screen) => screen.contains(flipped.center()),
                Place::Anywhere | Place::Nowhere => true,
            });
            match flip {
                Some((_, presented)) => shown.push(Shown {
                    id,
                    callbacks,
                    presented: *presented,
                }),
                None => waiting.push((id, callbacks)),
            }
        }
        self.painted = waiting;
        shown
    }
}

impl State {
    pub(crate) fn take_feedback(&self, id: WindowId) -> Callbacks {
        self.layers(id)
            .into_iter()
            .flat_map(|layer| {
                with_states(&layer.surface, |states| {
                    std::mem::take(
                        &mut states
                            .cached_state
                            .get::<PresentationFeedbackCachedState>()
                            .current()
                            .callbacks,
                    )
                })
            })
            .collect()
    }

    pub(crate) fn present(&self, shown: Shown) {
        let Shown {
            id,
            callbacks,
            presented,
        } = shown;
        self.send_frames(id, presented.time);
        let refresh = match presented.refresh.is_zero() {
            true => Refresh::Unknown,
            false => Refresh::fixed(presented.refresh),
        };
        let Some(output) = self.output_of(id) else {
            return;
        };
        for callback in callbacks {
            callback.presented(
                &output,
                presented.time,
                refresh,
                presented.sequence,
                presented.kind(),
            );
        }
    }
}

delegate_presentation!(State);
