use std::time::{Duration, Instant};

use crate::Undo;

pub const EDIT_BURST: Duration = Duration::from_millis(750);
pub const HISTORY_STEPS: usize = 200;

pub trait Moment: Copy {
    fn within(self, earlier: Self, burst: Duration) -> bool;
}

impl Moment for Instant {
    fn within(self, earlier: Self, burst: Duration) -> bool {
        self.saturating_duration_since(earlier) <= burst
    }
}

impl Moment for u64 {
    fn within(self, earlier: Self, burst: Duration) -> bool {
        u128::from(self.saturating_sub(earlier)) <= burst.as_millis()
    }
}

struct Gesture<C: Undo> {
    origin: Option<u64>,
    step: Option<C::Step>,
}

/// When a step's last edit was made, and by whom, while later edits may still absorb into it.
struct Burst<T> {
    at: T,
    origin: Option<u64>,
}

pub struct History<C: Undo, T: Moment = Instant> {
    undo: Vec<(C::Step, Option<Burst<T>>)>,
    redo: Vec<C::Step>,
    gesture: Option<Gesture<C>>,
    stopped: bool,
}

impl<C: Undo, T: Moment> Default for History<C, T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            stopped: false,
        }
    }
}

impl<C: Undo, T: Moment> History<C, T> {
    pub fn record(&mut self, content: &C, operation: &C::Op, origin: Option<u64>, at: Option<T>) {
        let Some(step) = content.step(operation) else {
            return;
        };
        self.redo.clear();
        if let Some(gesture) = &mut self.gesture
            && gesture.origin == origin
        {
            match &mut gesture.step {
                Some(held) => C::then(held, step),
                None => gesture.step = Some(step),
            }
            return;
        }
        let step = match (self.undo.last_mut(), at) {
            (Some((previous, Some(burst))), Some(now))
                if burst.origin == origin && now.within(burst.at, EDIT_BURST) =>
            {
                match C::absorb(previous, step) {
                    Ok(()) => {
                        burst.at = now;
                        return;
                    }
                    Err(step) => step,
                }
            }
            _ => step,
        };
        self.push((step, at.map(|at| Burst { at, origin })));
    }

    pub fn begin(&mut self, origin: Option<u64>) {
        self.close();
        self.gesture = Some(Gesture { origin, step: None });
    }

    pub fn commit(&mut self, origin: Option<u64>) {
        if self
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.origin == origin)
        {
            self.close();
        }
    }

    /// Ends the burst `origin`'s last edit went into, so its next edit starts a step of its
    /// own however soon it comes. A gesture `origin` holds open is left alone: the gesture
    /// already makes one step, and its commit or cancel says where that ends.
    pub fn split(&mut self, origin: Option<u64>) {
        if self
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.origin == origin)
        {
            return;
        }
        if let Some((_, burst)) = self.undo.last_mut()
            && burst.as_ref().is_some_and(|burst| burst.origin == origin)
        {
            *burst = None;
        }
    }

    pub fn cancel(&mut self, content: &C, origin: Option<u64>) -> Vec<C::Op> {
        let Some(gesture) = self.gesture.take_if(|gesture| gesture.origin == origin) else {
            return Vec::new();
        };
        gesture
            .step
            .map(|step| content.revert(&step))
            .unwrap_or_default()
    }

    pub fn undo(&mut self, content: &C) -> Vec<C::Op> {
        self.close();
        let Some((step, _)) = self.undo.pop() else {
            return Vec::new();
        };
        let operations = content.revert(&step);
        self.redo.push(step);
        operations
    }

    pub fn redo(&mut self, content: &C) -> Vec<C::Op> {
        self.close();
        let Some(step) = self.redo.pop() else {
            return Vec::new();
        };
        let operations = content.reapply(&step);
        self.undo.push((step, None));
        operations
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.gesture = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
            || self
                .gesture
                .as_ref()
                .is_some_and(|gesture| gesture.step.is_some())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The session restarted: the content was loaded afresh, so no step recorded before can
    /// apply. Undo and redo stop here, and `stopped` says there were steps behind it.
    pub fn restart(&mut self) {
        self.close();
        self.stopped |= !self.undo.is_empty() || !self.redo.is_empty();
        self.undo.clear();
        self.redo.clear();
    }

    pub fn gesture_open(&self) -> bool {
        self.gesture.is_some()
    }

    /// Whether undo or redo, once out of steps, stopped at a restart rather than at the start.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    fn close(&mut self) {
        if let Some(step) = self.gesture.take().and_then(|gesture| gesture.step) {
            self.push((step, None));
        }
    }

    fn push(&mut self, step: (C::Step, Option<Burst<T>>)) {
        self.undo.push(step);
        if self.undo.len() > HISTORY_STEPS {
            self.undo.remove(0);
        }
    }
}
