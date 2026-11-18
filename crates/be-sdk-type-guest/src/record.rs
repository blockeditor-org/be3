use be_block_content::{
    LiveEdit, Undo,
    history::{GestureId, GestureKind, History, Stepped},
};

pub trait Recorder<C: LiveEdit>: Default {
    fn record(
        &mut self,
        content: &C,
        operation: &C::Op,
        origin: Option<u64>,
        gesture: Option<u64>,
        at: u64,
    ) {
        let _ = (content, operation, origin, gesture, at);
    }

    fn begin(&mut self, gesture: GestureId, kind: GestureKind) {
        let _ = (gesture, kind);
    }

    fn commit(&mut self, gesture: GestureId) {
        let _ = gesture;
    }

    fn split(&mut self, origin: Option<u64>) {
        let _ = origin;
    }

    fn void(&mut self, gesture: GestureId) {
        let _ = gesture;
    }

    fn cancel(&mut self, content: &C, gesture: GestureId) -> Vec<C::Op> {
        let _ = (content, gesture);
        Vec::new()
    }

    fn undo(&mut self, content: &C) -> Stepped<C::Op> {
        let _ = content;
        Stepped::default()
    }

    fn redo(&mut self, content: &C) -> Stepped<C::Op> {
        let _ = content;
        Stepped::default()
    }

    fn clear(&mut self) {}

    fn restart(&mut self) {}

    fn stopped(&self) -> bool {
        false
    }

    fn gesture_open(&self) -> bool {
        false
    }

    fn can_undo(&self) -> bool {
        false
    }

    fn can_redo(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct NoHistory;

impl<C: LiveEdit> Recorder<C> for NoHistory {}

impl<C: Undo> Recorder<C> for History<C, u64> {
    fn record(
        &mut self,
        content: &C,
        operation: &C::Op,
        origin: Option<u64>,
        gesture: Option<u64>,
        at: u64,
    ) {
        History::record(self, content, operation, origin, gesture, Some(at));
    }

    fn begin(&mut self, gesture: GestureId, kind: GestureKind) {
        History::begin(self, gesture, kind);
    }

    fn commit(&mut self, gesture: GestureId) {
        History::commit(self, gesture);
    }

    fn split(&mut self, origin: Option<u64>) {
        History::split(self, origin);
    }

    fn void(&mut self, gesture: GestureId) {
        History::void(self, gesture);
    }

    fn cancel(&mut self, content: &C, gesture: GestureId) -> Vec<C::Op> {
        History::cancel(self, content, gesture)
    }

    fn undo(&mut self, content: &C) -> Stepped<C::Op> {
        History::undo(self, content)
    }

    fn redo(&mut self, content: &C) -> Stepped<C::Op> {
        History::redo(self, content)
    }

    fn clear(&mut self) {
        History::clear(self);
    }

    fn restart(&mut self) {
        History::restart(self);
    }

    fn stopped(&self) -> bool {
        History::stopped(self)
    }

    fn gesture_open(&self) -> bool {
        History::gesture_open(self)
    }

    fn can_undo(&self) -> bool {
        History::can_undo(self)
    }

    fn can_redo(&self) -> bool {
        History::can_redo(self)
    }
}
