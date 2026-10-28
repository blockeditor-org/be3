mod board;

use std::any::Any;
use std::rc::Rc;

use beui_types::layout::Direction;
use beui_types::node::{NodeId, NodeRef};
use beui_types::{Pos2, Rect, WindowId};

pub use beui_types::drag::DragPoint;
pub use board::Board;

pub trait Surface {
    type Order: Ord;

    fn placed(&self, node: NodeId) -> Option<(Rect, f32)>;

    fn visible(&self, node: NodeId) -> Option<Rect>;

    fn window(&self, node: NodeId) -> WindowId;

    fn target_order(&self, node: NodeId) -> Self::Order;

    fn scroller_order(&self, node: NodeId, visible: Rect) -> Self::Order;

    fn carrying(&self, _carrying: bool) {}
}

pub type Accepts = Rc<dyn Fn(&dyn Any) -> bool>;
pub type Dropped = Rc<dyn Fn(&dyn Any, DragPoint)>;
pub type Over = Rc<dyn Fn(Option<(&dyn Any, DragPoint)>)>;
pub type Pending = Rc<dyn Fn(DragPoint)>;
pub type Outside = Rc<dyn Fn(DragPoint)>;

pub struct Target {
    pub id: u64,
    pub node: NodeRef,
    pub accepts: Accepts,
    pub takes: Rc<dyn Fn(DragPoint) -> bool>,
    pub carrying: Rc<dyn Fn(bool)>,
    pub over: Over,
    pub dropped: Dropped,
    pub hides_preview: Rc<dyn Fn() -> bool>,
    pub holds: Accepts,
}

pub struct DragSource {
    pub payload: Rc<dyn Any>,
    pub item: Rect,
    pub follow: Rc<dyn Fn(DragPoint)>,
    pub floating: Rc<dyn Fn(bool)>,
    pub outside: Outside,
    pub cancelled: Rc<dyn Fn()>,
    pub dropped: Rc<dyn Fn()>,
}

impl DragSource {
    pub fn new(payload: Rc<dyn Any>) -> Self {
        Self {
            payload,
            item: Rect::ZERO,
            follow: Rc::new(|_| {}),
            floating: Rc::new(|_| {}),
            outside: Rc::new(|_| {}),
            cancelled: Rc::new(|| {}),
            dropped: Rc::new(|| {}),
        }
    }
}

pub const EDGE_BAND: f32 = 40.0;

pub struct EdgeScroller {
    pub id: u64,
    pub node: NodeRef,
    pub axis: Rc<dyn Fn() -> Direction>,
    pub movable: Rc<dyn Fn() -> [bool; 2]>,
    pub scroll: Rc<dyn Fn(Option<f32>)>,
}

impl EdgeScroller {
    pub fn depth(&self, rect: Rect, pos: Pos2) -> Option<f32> {
        let axis = (self.axis)();
        let along = |pos: Pos2| axis.main(pos.to_vec2());
        let (start, end) = (along(rect.min), along(rect.max));
        let band = EDGE_BAND.min((end - start) / 4.0);
        let at = along(pos);
        let movable = (self.movable)();
        match () {
            _ if at < start + band && movable[0] => Some(at - (start + band)),
            _ if at > end - band && movable[1] => Some(at - (end - band)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
