//! The states a host sent an app, by the `configure` serial they went with, so a frame is read
//! against the state it was laid out for (guides/apps/runtime.md, Layout and present commits).
use std::collections::VecDeque;

const KEPT: usize = 64;

#[derive(Debug)]
pub struct Sent<T> {
    states: VecDeque<(u64, T)>,
}

impl<T> Default for Sent<T> {
    fn default() -> Self {
        Self {
            states: VecDeque::new(),
        }
    }
}

impl<T: PartialEq> Sent<T> {
    pub fn push(&mut self, serial: u64, state: T) {
        if self.states.back().is_some_and(|(_, last)| *last == state) {
            return;
        }
        if self.states.len() == KEPT {
            self.states.pop_front();
        }
        self.states.push_back((serial, state));
    }

    pub fn at(&self, serial: u64) -> Option<&T> {
        self.states
            .iter()
            .rev()
            .find(|(sent, _)| *sent <= serial)
            .map(|(_, state)| state)
    }

    pub fn confirm(&mut self, serial: u64) {
        while self.states.get(1).is_some_and(|(next, _)| *next <= serial) {
            self.states.pop_front();
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests;
