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

/// A gesture, named by the editor that opened it and an id of that editor's, so one editor can
/// hold several at once (two fingers, two controllers) and each ends on its own.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GestureId {
    pub editor: Option<u64>,
    pub id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GestureKind {
    /// Groups its edits into one undo step. They are written as they happen.
    Group,
    /// Its edits are held until it commits (decisions/tentative_edits_wait_for_release.md). The
    /// open tentative gestures form one layer, which becomes one undo step when the last of them
    /// ends.
    Tentative,
}

/// What an undo or redo did: the operations that step it, or, when an undo found a tentative
/// layer held, the open tentative gestures it ended. A non-empty `cancelled` means the caller
/// drops its whole layer, with the edits of gestures that committed and were waiting for the
/// rest; `cancelled` names only the open ones, which are told their gesture ended.
#[must_use]
pub struct Stepped<Op> {
    pub operations: Vec<Op>,
    pub cancelled: Vec<GestureId>,
    /// The undo groups the undo or redo committed first, each a step of its own now: their
    /// editors are told they ended.
    pub committed: Vec<GestureId>,
}

impl<Op> Default for Stepped<Op> {
    fn default() -> Self {
        Self {
            operations: Vec::new(),
            cancelled: Vec::new(),
            committed: Vec::new(),
        }
    }
}

impl<Op> Stepped<Op> {
    pub fn changed(&self) -> bool {
        !self.operations.is_empty() || !self.cancelled.is_empty()
    }
}

/// When a step's last edit was made, and by whom, while later edits may still absorb into it.
struct Burst<T> {
    at: T,
    origin: Option<u64>,
}

pub struct History<C: Undo, T: Moment = Instant> {
    undo: Vec<(C::Step, Option<Burst<T>>)>,
    redo: Vec<C::Step>,
    /// Open undo groups, in the order they began.
    groups: Vec<(GestureId, Option<C::Step>)>,
    /// Open tentative gestures, in the order they began.
    tentative: Vec<GestureId>,
    /// The tentative layer's steps in the order its edits were made, a gesture's consecutive
    /// edits joined, kept until the last open tentative gesture ends. A committed gesture's
    /// steps stay here until then; a cancelled one's go.
    held: Vec<(GestureId, C::Step)>,
    stopped: bool,
}

impl<C: Undo, T: Moment> Default for History<C, T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            groups: Vec::new(),
            tentative: Vec::new(),
            held: Vec::new(),
            stopped: false,
        }
    }
}

impl<C: Undo, T: Moment> History<C, T> {
    /// Records `operation`, about to be applied to `content`, made by `origin` in its gesture
    /// `gesture`, if any. An edit naming a gesture that is not open is recorded as one made
    /// outside any.
    pub fn record(
        &mut self,
        content: &C,
        operation: &C::Op,
        origin: Option<u64>,
        gesture: Option<u64>,
        at: Option<T>,
    ) {
        let Some(step) = content.step(operation) else {
            return;
        };
        let gesture = gesture.map(|id| GestureId { editor: origin, id });
        if let Some(gesture) = gesture
            && self.tentative.contains(&gesture)
        {
            match self.held.last_mut() {
                Some((last, held)) if *last == gesture => C::then(held, step),
                _ => self.held.push((gesture, step)),
            }
            return;
        }
        self.redo.clear();
        if let Some(gesture) = gesture
            && let Some((_, held)) = self.groups.iter_mut().find(|(open, _)| *open == gesture)
        {
            match held {
                Some(held) => C::then(held, step),
                None => *held = Some(step),
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

    /// Opens `gesture`, which must not be open: a caller given a begin under an open name
    /// commits that gesture first, so it sees a layer that ends and writes it. Every other open
    /// gesture stays open.
    pub fn begin(&mut self, gesture: GestureId, kind: GestureKind) {
        debug_assert!(
            !self.groups.iter().any(|(open, _)| *open == gesture)
                && !self.tentative.contains(&gesture),
            "{gesture:?} is already open"
        );
        match kind {
            GestureKind::Group => self.groups.push((gesture, None)),
            GestureKind::Tentative => self.tentative.push(gesture),
        }
    }

    /// Ends `gesture`, keeping its edits. An undo group becomes a step of its own; a tentative
    /// gesture's edits wait for the rest of the layer, and the layer becomes one step when its
    /// last gesture ends.
    pub fn commit(&mut self, gesture: GestureId) {
        if let Some(at) = self.groups.iter().position(|(open, _)| *open == gesture) {
            let (_, step) = self.groups.remove(at);
            if let Some(step) = step {
                self.push((step, None));
            }
        } else if let Some(at) = self.tentative.iter().position(|open| *open == gesture) {
            self.tentative.remove(at);
            self.settle_layer();
        }
    }

    /// Ends `gesture`, leaving no step of it. For an undo group, the operations that take its
    /// edits back. A tentative gesture's edits were never written, so the caller drops them from
    /// its layer: nothing is returned for one.
    pub fn cancel(&mut self, content: &C, gesture: GestureId) -> Vec<C::Op> {
        if let Some(at) = self.groups.iter().position(|(open, _)| *open == gesture) {
            let (_, step) = self.groups.remove(at);
            return step.map(|step| content.revert(&step)).unwrap_or_default();
        }
        if let Some(at) = self.tentative.iter().position(|open| *open == gesture) {
            self.tentative.remove(at);
            self.held.retain(|(of, _)| *of != gesture);
            self.settle_layer();
        }
        Vec::new()
    }

    /// Drops what the tentative gesture `gesture` recorded, open or committed and waiting for
    /// the rest of the layer, and keeps it open if it is: its edits could not be kept (the
    /// content they moved is gone), but it ends only when its editor ends it.
    pub fn void(&mut self, gesture: GestureId) {
        self.held.retain(|(of, _)| *of != gesture);
    }

    /// Ends the burst `origin`'s last edit went into, so its next edit starts a step of its
    /// own however soon it comes. An edit made in an open gesture goes into that gesture
    /// whatever the burst.
    pub fn split(&mut self, origin: Option<u64>) {
        if let Some((_, burst)) = self.undo.last_mut()
            && burst.as_ref().is_some_and(|burst| burst.origin == origin)
        {
            *burst = None;
        }
    }

    /// Takes back the last step. While a tentative layer is held, cancels the whole layer
    /// instead, as Escape would, and leaves the steps alone. Open undo groups commit first.
    pub fn undo(&mut self, content: &C) -> Stepped<C::Op> {
        if self.holding() {
            self.held.clear();
            return Stepped {
                cancelled: std::mem::take(&mut self.tentative),
                ..Stepped::default()
            };
        }
        let committed = self.close_groups();
        let operations = match self.undo.pop() {
            Some((step, _)) => {
                let operations = content.revert(&step);
                self.redo.push(step);
                operations
            }
            None => Vec::new(),
        };
        Stepped {
            operations,
            committed,
            ..Stepped::default()
        }
    }

    /// Makes the last step undone again. Refused, doing nothing, while a tentative layer is
    /// held. Open undo groups commit first.
    pub fn redo(&mut self, content: &C) -> Stepped<C::Op> {
        let mut committed = Vec::new();
        let operations = match self.holding() {
            true => Vec::new(),
            false => {
                committed = self.close_groups();
                match self.redo.pop() {
                    Some(step) => {
                        let operations = content.reapply(&step);
                        self.undo.push((step, None));
                        operations
                    }
                    None => Vec::new(),
                }
            }
        };
        Stepped {
            operations,
            committed,
            ..Stepped::default()
        }
    }

    /// Drops every step and every open gesture: the content was replaced, so none of them
    /// applies.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.groups.clear();
        self.tentative.clear();
        self.held.clear();
    }

    pub fn can_undo(&self) -> bool {
        self.holding()
            || !self.undo.is_empty()
            || self.groups.iter().any(|(_, step)| step.is_some())
    }

    pub fn can_redo(&self) -> bool {
        !self.holding() && !self.redo.is_empty()
    }

    /// The session restarted: the content was loaded afresh, so no step recorded before can
    /// apply. Undo and redo stop here, and `stopped` says there were steps behind it. Open
    /// gestures stay open with what they recorded, since their edits are carried onto the
    /// reloaded content: each still commits as one step.
    pub fn restart(&mut self) {
        self.stopped |= !self.undo.is_empty() || !self.redo.is_empty();
        self.undo.clear();
        self.redo.clear();
    }

    pub fn gesture_open(&self) -> bool {
        !self.groups.is_empty() || !self.tentative.is_empty()
    }

    /// Whether a tentative gesture is open, so the layer's edits are held.
    pub fn holding(&self) -> bool {
        !self.tentative.is_empty()
    }

    /// The open tentative gestures, in the order they began.
    pub fn tentative(&self) -> &[GestureId] {
        &self.tentative
    }

    /// Whether undo or redo, once out of steps, stopped at a restart rather than at the start.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Commits every open undo group, returning them.
    fn close_groups(&mut self) -> Vec<GestureId> {
        let mut closed = Vec::new();
        for (gesture, step) in std::mem::take(&mut self.groups) {
            if let Some(step) = step {
                self.push((step, None));
            }
            closed.push(gesture);
        }
        closed
    }

    /// Once no tentative gesture is open, what the layer kept becomes one step.
    fn settle_layer(&mut self) {
        if self.holding() {
            return;
        }
        let mut held = std::mem::take(&mut self.held).into_iter();
        let Some((_, mut step)) = held.next() else {
            return;
        };
        for (_, next) in held {
            C::then(&mut step, next);
        }
        self.redo.clear();
        self.push((step, None));
    }

    fn push(&mut self, step: (C::Step, Option<Burst<T>>)) {
        self.undo.push(step);
        if self.undo.len() > HISTORY_STEPS {
            self.undo.remove(0);
        }
    }
}

#[cfg(test)]
mod tests;
