use std::any::Any;
use std::rc::Rc;

pub use crate::types::drag::{DragPoint, Outside, Pending, Target};

#[derive(Clone)]
pub struct Board(Rc<crate::core::drag::Board>);

impl Board {
    pub fn register(&self, target: Target) -> u64 {
        self.0.register(target)
    }

    pub fn unregister(&self, id: u64) {
        self.0.unregister(id);
    }

    pub fn carrying(&self) -> bool {
        self.0.carrying()
    }

    pub fn begin(
        &self,
        payload: Rc<dyn Any>,
        point: DragPoint,
        follow: Rc<dyn Fn(DragPoint)>,
        outside: Outside,
    ) {
        self.0.begin(payload, point, follow, outside);
    }

    pub fn press(&self, pending: Pending) {
        self.0.press(pending);
    }

    pub fn release(&self) {
        self.0.release();
    }

    pub fn track(&self, point: DragPoint) {
        self.0.track(point);
    }

    pub fn finish(&self) {
        self.0.finish();
    }

    pub fn end(&self, point: Option<DragPoint>) {
        self.0.end(point);
    }
}

pub fn drag_board() -> Board {
    Board(crate::core::drag::drag_board())
}
