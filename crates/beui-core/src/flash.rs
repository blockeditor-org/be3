use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::color::Color32;

pub const LIFETIME: Duration = Duration::from_millis(700);
pub const CHANGE: Color32 = Color32::from_rgb(126, 231, 135);
pub const REPAINT: Color32 = Color32::from_rgb(255, 122, 190);

const CAPACITY: usize = 512;

pub struct FlashLog<T> {
    enabled: bool,
    entries: VecDeque<(T, Instant)>,
}

impl<T> Default for FlashLog<T> {
    fn default() -> Self {
        Self {
            enabled: false,
            entries: VecDeque::new(),
        }
    }
}

impl<T> FlashLog<T> {
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.entries.clear();
    }

    pub fn record(&mut self, value: T, at: Instant) {
        if !self.enabled {
            return;
        }
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((value, at));
    }

    pub fn prune(&mut self, now: Instant) {
        while self
            .entries
            .front()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= LIFETIME)
        {
            self.entries.pop_front();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&T, Instant)> {
        self.entries.iter().map(|(value, at)| (value, *at))
    }
}

pub fn remaining(now: Instant, at: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(at).as_secs_f32();
    (1.0 - elapsed / LIFETIME.as_secs_f32()).clamp(0.0, 1.0)
}
