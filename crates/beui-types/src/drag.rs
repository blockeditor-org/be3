use be_lib_geometry::{Pos2, Rect, Vec2};

use crate::input::{Modifiers, PointerId, PointerPress};
use crate::window::WindowId;

mod spec;
pub use spec::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragPoint {
    pub window: WindowId,
    pub pos: Pos2,
    pub local: Pos2,
    pub size: Vec2,
    pub fraction: Vec2,
    pub modifiers: Modifiers,
    pub item: Rect,
    /// Which pointer carries: a board follows one carry per pointer.
    pub pointer: PointerId,
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
            pointer: press.pointer,
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
            pointer: PointerId::Mouse,
        }
    }

    pub fn by(self, pointer: PointerId) -> Self {
        Self { pointer, ..self }
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

/// How far from a scroller's edge a carried item starts scrolling it.
pub const EDGE_BAND: f32 = 40.0;

/// A scroll area a carried item scrolls while it is held near an edge: `scroll` hears how
/// far into the band the pointer is along each axis (negative toward the start), or None
/// when it stops. `movable` says which ways it can move now, along x and then y, toward the
/// start and toward the end: a list scrolls along one axis, a canvas's world along both.
pub struct EdgeScroller {
    pub id: u64,
    pub node: crate::node::NodeRef,
    pub movable: std::rc::Rc<dyn Fn() -> [[bool; 2]; 2]>,
    pub scroll: std::rc::Rc<dyn Fn(Option<Vec2>)>,
}

impl EdgeScroller {
    pub fn depth(&self, rect: Rect, pos: Pos2) -> Option<Vec2> {
        let movable = (self.movable)();
        let axis = |at: f32, start: f32, end: f32, movable: [bool; 2]| {
            let band = EDGE_BAND.min((end - start) / 4.0);
            match () {
                _ if at < start + band && movable[0] => at - (start + band),
                _ if at > end - band && movable[1] => at - (end - band),
                _ => 0.0,
            }
        };
        let depth = Vec2::new(
            axis(pos.x, rect.min.x, rect.max.x, movable[0]),
            axis(pos.y, rect.min.y, rect.max.y, movable[1]),
        );
        (depth != Vec2::ZERO).then_some(depth)
    }
}

/// Where a core's nodes are: their rect and scale, and the part of them that shows.
pub trait Space {
    fn placed(&self, node: crate::node::NodeId) -> Option<(Rect, f32)>;
    fn visible(&self, node: crate::node::NodeId) -> Option<Rect>;
}

/// A node's paint order and window, as the core that tracks a drag knows them.
pub type Order<'a> = &'a dyn Fn(crate::node::NodeId) -> (Vec<usize>, WindowId);

/// The drag board as a core drives it: pointer points, releases, Escape, and the pointer
/// leaving the window. It holds a carry per pointer: each pointer's points move and release
/// only its own.
pub trait Driver {
    /// Whether anything is carried.
    fn carrying(&self) -> bool;
    fn set_outside(&self, outside: bool);
    fn track(&self, point: DragPoint, order: Order);
    /// `pointer` let go (or lost its press); a carry held by another pointer goes on.
    fn release(&self, pointer: PointerId, cancelled: bool, order: Order);
    /// Puts down every carry (Escape); whether there was one.
    fn cancel(&self) -> bool;
    /// Stops every carry's edge scrolling: the core has lost the pointer's position.
    fn stop_edge(&self);
    /// Whether `pointer` may lift a carry now: it carries none already.
    fn lifts(&self, pointer: PointerId) -> bool;
    /// Lifts `carry` at `point`, a drag an `Interactive`'s `drag` began: from here the
    /// carried item chooses among the places the drop sites that accept it give. `None`
    /// when `point`'s pointer carries something already.
    fn lift(&self, carry: Carry, point: DragPoint) -> Option<CarryId>;
    /// Puts `carry` down without dropping it, if it is still carried: its source is gone.
    fn put_down(&self, carry: CarryId);
    /// Whether `carry` is still carried; tracked.
    fn holds(&self, carry: CarryId) -> bool;
    /// The frame `carry` shows, while it is carried; tracked.
    fn frame(&self, carry: CarryId) -> Option<DragFrame>;
    /// Moves the keyboard's carry to the nearest place `toward` from where it is; whether one
    /// was there.
    fn step(&self, toward: Vec2) -> bool;
    /// What the last carry said for a screen reader (lifted, each place a step chooses, put
    /// down), and which carry; tracked.
    fn said(&self) -> Option<(CarryId, String)>;
}
