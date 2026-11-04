use std::rc::Rc;

pub use crate::types::drag::{
    Accepts, DragPoint, DragSource, Driver, Dropped, EDGE_BAND, EdgeScroller, Order, Outside, Over,
    Pending, Space, Target,
};

pub fn space() -> Rc<dyn Space> {
    crate::core::drag::space()
}

pub fn drive(driver: Rc<dyn Driver>) {
    crate::core::drag::drive(driver);
}
