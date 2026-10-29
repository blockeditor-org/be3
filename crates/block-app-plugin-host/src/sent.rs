use std::collections::VecDeque;

const KEPT: usize = 64;

#[derive(Debug)]
pub(super) struct Sent<T> {
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
    pub(super) fn push(&mut self, serial: u64, state: T) {
        if self.states.back().is_some_and(|(_, last)| *last == state) {
            return;
        }
        if self.states.len() == KEPT {
            self.states.pop_front();
        }
        self.states.push_back((serial, state));
    }

    pub(super) fn at(&self, serial: u64) -> Option<&T> {
        self.states
            .iter()
            .rev()
            .find(|(sent, _)| *sent <= serial)
            .map(|(_, state)| state)
    }

    pub(super) fn confirm(&mut self, serial: u64) {
        while self.states.get(1).is_some_and(|(next, _)| *next <= serial) {
            self.states.pop_front();
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests;
