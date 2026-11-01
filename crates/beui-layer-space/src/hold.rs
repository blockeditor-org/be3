use std::time::{Duration, Instant};

use beui_types::{Key, Keystroke};

pub const LET_GO_AFTER: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Default, Debug)]
pub struct Hold {
    since: Option<Instant>,
    fired: bool,
}

impl Hold {
    pub fn key(&mut self, keystroke: &Keystroke, now: Option<Instant>, grabbed: bool) -> bool {
        if keystroke.key != Some(Key::Escape) {
            return true;
        }
        match (keystroke.pressed, keystroke.repeat) {
            (true, false) => {
                self.since = now.filter(|_| grabbed);
                self.fired = false;
                true
            }
            (true, true) => !self.fired,
            (false, _) => {
                *self = Self::default();
                true
            }
        }
    }

    pub fn due(&mut self, now: Instant) -> bool {
        match self.since {
            Some(since) if since + LET_GO_AFTER <= now => {
                self.since = None;
                self.fired = true;
                true
            }
            _ => false,
        }
    }

    pub fn due_in(&self, now: Option<Instant>) -> Duration {
        match (self.since, now) {
            (Some(since), Some(now)) => (since + LET_GO_AFTER).saturating_duration_since(now),
            _ => Duration::MAX,
        }
    }

    pub fn cancel(&mut self) {
        self.since = None;
    }
}
