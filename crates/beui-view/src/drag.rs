use std::rc::Rc;

pub use crate::types::drag::{
    Accepts, DragPoint, DragSource, Driver, Dropped, EDGE_BAND, EdgeScroller, Order, Outside, Over,
    Pending, Space, Target,
};
pub use crate::types::drag::{
    After, Candidate, CandidateId, Carry, CarryFace, CarryId, DragFrame, DragSpec, Evaluated,
    Evaluation, REACH, Rest, SNAP_RADIUS, STICKY, THROW_SECONDS, THROW_SPEED, closest, custom,
    evaluate, fixed, floating, free, into, move_after, nodes, rests, step_toward,
};

pub fn space() -> Rc<dyn Space> {
    crate::core::drag::space()
}

pub fn drive(driver: Rc<dyn Driver>) {
    crate::core::drag::drive(driver);
}

/// The driver the tree's drags go to, if one was given to `drive`.
pub fn driver() -> Option<Rc<dyn Driver>> {
    crate::core::drag::driver()
}
