//! How a host gives up on something it starts again after each failure (a type module, the
//! input remaps, Xwayland): so many failures within a window, then it stops, until whoever owns
//! it starts afresh (`reset`). Each failure is told when it happened, on whichever clock its
//! caller keeps, so tests move it rather than waiting.

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FailureBudget {
    allowed: usize,
    window: Duration,
    failures: Vec<Duration>,
}

/// What a failure leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failed {
    /// Start it again.
    Again,
    /// It failed `allowed` times within the window: leave it stopped.
    Stop,
}

impl FailureBudget {
    /// Stops at the `allowed`th failure within any `window`.
    pub const fn new(allowed: usize, window: Duration) -> Self {
        Self {
            allowed,
            window,
            failures: Vec::new(),
        }
    }

    pub const fn allowed(&self) -> usize {
        self.allowed
    }

    pub const fn window(&self) -> Duration {
        self.window
    }

    /// Counts a failure at `now`, forgetting those longer than the window before it.
    pub fn fail(&mut self, now: Duration) -> Failed {
        let window = self.window;
        self.failures
            .retain(|at| now.saturating_sub(*at) < window);
        self.failures.push(now);
        match self.failures.len() >= self.allowed {
            true => Failed::Stop,
            false => Failed::Again,
        }
    }

    /// Forgets every failure, for something started afresh.
    pub fn reset(&mut self) {
        self.failures.clear();
    }
}

#[cfg(test)]
mod tests;
