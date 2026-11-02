use std::rc::Rc;

use beui::reactive::*;
use beui::{ForwardedInput, WindowEdge};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    Move,
    Resize(WindowEdge),
}

#[derive(Clone)]
pub struct Carrier {
    pub begin: Rc<dyn Fn(Carry) -> bool>,
    pub steer: Rc<dyn Fn(ForwardedInput) -> ForwardedInput>,
}

#[derive(Clone)]
pub struct Carried(pub Option<Carrier>);

pub fn carrier() -> Option<Carrier> {
    use_context::<Carried>().and_then(|Carried(carrier)| carrier)
}
