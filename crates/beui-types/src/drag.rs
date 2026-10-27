use geometry::{Pos2, Rect, Vec2};

use crate::input::{Modifiers, PointerPress};
use crate::window::WindowId;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragPoint {
    pub window: WindowId,
    pub pos: Pos2,
    pub local: Pos2,
    pub size: Vec2,
    pub fraction: Vec2,
    pub modifiers: Modifiers,
    pub item: Rect,
}

impl DragPoint {
    pub fn of(press: PointerPress) -> Self {
        Self {
            window: press.window,
            pos: press.pos,
            local: press.local,
            size: press.size,
            fraction: press.fraction,
            modifiers: press.modifiers,
            item: Rect::ZERO,
        }
    }

    pub fn at(pos: Pos2, modifiers: Modifiers) -> Self {
        Self {
            window: WindowId::MAIN,
            pos,
            local: pos,
            size: Vec2::ZERO,
            fraction: Vec2::ZERO,
            modifiers,
            item: Rect::ZERO,
        }
    }

    pub fn in_window(self, window: WindowId) -> Self {
        Self { window, ..self }
    }

    pub fn within(self, (rect, scale): (Rect, f32)) -> Self {
        let local = (self.pos - rect.min) / scale;
        let axis = |offset: f32, length: f32| {
            if length > 0.0 {
                (offset / length).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Self {
            local: Pos2::new(local.x, local.y),
            size: rect.size() / scale,
            fraction: Vec2::new(
                axis(self.pos.x - rect.left(), rect.width()),
                axis(self.pos.y - rect.top(), rect.height()),
            ),
            ..self
        }
    }
}

pub type Accepts = std::rc::Rc<dyn Fn(&dyn std::any::Any) -> bool>;
pub type Dropped = std::rc::Rc<dyn Fn(&dyn std::any::Any, DragPoint)>;
pub type Over = std::rc::Rc<dyn Fn(Option<(&dyn std::any::Any, DragPoint)>)>;
pub type Pending = std::rc::Rc<dyn Fn(DragPoint)>;
pub type Outside = std::rc::Rc<dyn Fn(DragPoint)>;

pub struct Target {
    pub id: u64,
    pub node: crate::node::NodeRef,
    pub accepts: Accepts,
    pub takes: std::rc::Rc<dyn Fn(DragPoint) -> bool>,
    pub carrying: std::rc::Rc<dyn Fn(bool)>,
    pub over: Over,
    pub dropped: Dropped,
    pub hides_preview: std::rc::Rc<dyn Fn() -> bool>,
    pub holds: Accepts,
}

pub struct DragSource {
    pub payload: std::rc::Rc<dyn std::any::Any>,
    pub item: Rect,
    pub follow: std::rc::Rc<dyn Fn(DragPoint)>,
    pub floating: std::rc::Rc<dyn Fn(bool)>,
    pub outside: Outside,
    pub cancelled: std::rc::Rc<dyn Fn()>,
    pub dropped: std::rc::Rc<dyn Fn()>,
}

impl DragSource {
    pub fn new(payload: std::rc::Rc<dyn std::any::Any>) -> Self {
        Self {
            payload,
            item: Rect::ZERO,
            follow: std::rc::Rc::new(|_| {}),
            floating: std::rc::Rc::new(|_| {}),
            outside: std::rc::Rc::new(|_| {}),
            cancelled: std::rc::Rc::new(|| {}),
            dropped: std::rc::Rc::new(|| {}),
        }
    }
}

pub const EDGE_BAND: f32 = 40.0;

pub struct EdgeScroller {
    pub id: u64,
    pub node: crate::node::NodeRef,
    pub axis: std::rc::Rc<dyn Fn() -> crate::layout::Direction>,
    pub movable: std::rc::Rc<dyn Fn() -> [bool; 2]>,
    pub scroll: std::rc::Rc<dyn Fn(Option<f32>)>,
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

pub fn holder(targets: &[Target], payload: &dyn std::any::Any) -> Option<u64> {
    let holder = targets.iter().find(|target| (target.holds)(payload))?;
    let elsewhere = targets
        .iter()
        .any(|target| target.id != holder.id && (target.accepts)(payload));
    (!elsewhere).then_some(holder.id)
}
