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
    pub carrying: std::rc::Rc<dyn Fn(bool)>,
    pub over: Over,
    pub dropped: Dropped,
}
