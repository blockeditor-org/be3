use std::rc::Rc;

pub use crate::types::drag::{
    After, Candidate, CandidateId, Carry, CarryFace, CarryId, DragFrame, DragSpec, Evaluated,
    Evaluation, FREE_STEP, REACH, Rest, SNAP_RADIUS, STICKY, Stop, THROW_SECONDS, THROW_SPEED,
    closest, custom, evaluate, fixed, floating, free, free_step, into, move_after, nodes, rests,
    step_toward,
};
pub use crate::types::drag::{DragPoint, Driver, EDGE_BAND, EdgeScroller, Order, Space};

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
