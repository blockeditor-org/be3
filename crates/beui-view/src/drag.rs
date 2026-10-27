use std::rc::Rc;

pub use crate::types::drag::{
    DragPoint, DragSource, EDGE_BAND, EdgeScroller, Outside, Pending, Target,
};
use crate::types::Rect;

#[derive(Clone)]
pub struct Board(Rc<crate::core::drag::Board>);

impl Board {
    pub fn register(&self, target: Target) -> u64 {
        self.0.register(target)
    }

    pub fn unregister(&self, id: u64) {
        self.0.unregister(id);
    }

    pub fn register_scroller(&self, scroller: EdgeScroller) -> u64 {
        self.0.register_scroller(scroller)
    }

    pub fn unregister_scroller(&self, id: u64) {
        self.0.unregister_scroller(id);
    }

    pub fn carrying(&self) -> bool {
        self.0.carrying()
    }

    pub fn item(&self) -> Option<Rect> {
        self.0.item()
    }

    pub fn begin(&self, source: DragSource, point: DragPoint) {
        self.0.begin(source, point);
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

    pub fn end(&self, point: Option<DragPoint>) -> bool {
        self.0.end(point)
    }

    pub fn cancel(&self) -> bool {
        self.0.cancel()
    }
}

pub fn drag_board() -> Board {
    Board(crate::core::drag::drag_board())
}
