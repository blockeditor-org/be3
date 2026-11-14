use be_block_content::{LiveEdit, Undo, history::History};

pub trait Recorder<C: LiveEdit>: Default {
    fn record(&mut self, content: &C, operation: &C::Op, origin: Option<u64>, at: u64) {
        let _ = (content, operation, origin, at);
    }

    fn begin(&mut self, origin: Option<u64>) {
        let _ = origin;
    }

    fn commit(&mut self, origin: Option<u64>) {
        let _ = origin;
    }

    fn split(&mut self, origin: Option<u64>) {
        let _ = origin;
    }

    fn cancel(&mut self, content: &C, origin: Option<u64>) -> Vec<C::Op> {
        let _ = (content, origin);
        Vec::new()
    }

    fn undo(&mut self, content: &C) -> Vec<C::Op> {
        let _ = content;
        Vec::new()
    }

    fn redo(&mut self, content: &C) -> Vec<C::Op> {
        let _ = content;
        Vec::new()
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
    fn record(&mut self, content: &C, operation: &C::Op, origin: Option<u64>, at: u64) {
        History::record(self, content, operation, origin, Some(at));
    }

    fn begin(&mut self, origin: Option<u64>) {
        History::begin(self, origin);
    }

    fn commit(&mut self, origin: Option<u64>) {
        History::commit(self, origin);
    }

    fn split(&mut self, origin: Option<u64>) {
        History::split(self, origin);
    }

    fn cancel(&mut self, content: &C, origin: Option<u64>) -> Vec<C::Op> {
        History::cancel(self, content, origin)
    }

    fn undo(&mut self, content: &C) -> Vec<C::Op> {
        History::undo(self, content)
    }

    fn redo(&mut self, content: &C) -> Vec<C::Op> {
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
