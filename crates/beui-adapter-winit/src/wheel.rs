#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use winit::event::TouchPhase;

const MOMENTUM_GAP: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Wheeled {
    StartAndScroll,
    Scroll,
    ScrollAndEnd,
    Ignore,
}

#[derive(Clone, Copy, Default)]
enum State {
    #[default]
    Scrolling,
    Lifted(Instant),
    Coasting,
}

pub(crate) struct Wheel {
    system_momentum: bool,
    state: State,
}

impl Wheel {
    pub(crate) fn new(system_momentum: bool) -> Self {
        Self {
            system_momentum,
            state: State::default(),
        }
    }

    pub(crate) fn event(&mut self, phase: TouchPhase, now: Instant) -> Wheeled {
        match (phase, self.state) {
            (TouchPhase::Started, State::Lifted(lifted))
                if self.system_momentum
                    && now.saturating_duration_since(lifted) <= MOMENTUM_GAP =>
            {
                self.state = State::Coasting;
                Wheeled::Ignore
            }
            (TouchPhase::Started, _) => {
                self.state = State::Scrolling;
                Wheeled::StartAndScroll
            }
            (TouchPhase::Moved, State::Coasting) => Wheeled::Ignore,
            (TouchPhase::Moved, _) => Wheeled::Scroll,
            (TouchPhase::Ended | TouchPhase::Cancelled, State::Coasting) => {
                self.state = State::Scrolling;
                Wheeled::Ignore
            }
            (TouchPhase::Ended | TouchPhase::Cancelled, _) => {
                self.state = State::Lifted(now);
                Wheeled::ScrollAndEnd
            }
        }
    }
}
