use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::color::Color32;

pub(crate) const LIFETIME: Duration = Duration::from_millis(700);
pub(crate) const CHANGE: Color32 = Color32::from_rgb(126, 231, 135);
pub(crate) const REPAINT: Color32 = Color32::from_rgb(255, 122, 190);

const CAPACITY: usize = 512;

pub(crate) struct FlashLog<T> {
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
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.entries.clear();
    }

    pub(crate) fn record(&mut self, value: T, at: Instant) {
        if !self.enabled {
            return;
        }
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((value, at));
    }

    pub(crate) fn prune(&mut self, now: Instant) {
        while self
            .entries
            .front()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= LIFETIME)
        {
            self.entries.pop_front();
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&T, Instant)> {
        self.entries.iter().map(|(value, at)| (value, *at))
    }
}

pub(crate) fn remaining(now: Instant, at: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(at).as_secs_f32();
    (1.0 - elapsed / LIFETIME.as_secs_f32()).clamp(0.0, 1.0)
}
