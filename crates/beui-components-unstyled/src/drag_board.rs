use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_lib_stable_hash::HashMap;
use beui_view::drag::{CarryId, DragFrame};
use beui_view::drag::{DragPoint, Driver, EdgeScroller, Order, Space};
use beui_view::reactive::{Get, NodeId, OwnedSignal, tree_local};

mod sites;
use beui_view::types::input::PointerId;
use beui_view::types::{Rect, Vec2, WindowId};
pub(crate) use sites::{Erased, Site};

/// The drag board of the current tree: the carries held (one per pointer), the drop sites
/// they choose among, and the scrollers they scroll at an edge. Each core says where its
/// nodes are (`Space`) and drives the board with pointer points (`Driver`).
pub fn drag_board() -> Rc<Board> {
    tree_local(|| {
        let board = Rc::new(Board::new(beui_view::drag::space()));
        beui_view::drag::drive(Rc::clone(&board) as Rc<dyn Driver>);
        board
    })
}

/// The scroller a carry is scrolling: its id, how deep into its band, how to tell it, and
/// the pointer whose carry scrolls it (only that one moves or stops it).
type Scrolling = (u64, Vec2, Rc<dyn Fn(Option<Vec2>)>, PointerId);

pub struct Board {
    space: Rc<dyn Space>,
    orders: RefCell<HashMap<NodeId, (Vec<usize>, WindowId)>>,
    outside: Cell<bool>,
    scrollers: RefCell<Vec<EdgeScroller>>,
    scrolling: RefCell<Option<Scrolling>>,
    next: Cell<u64>,
    sites: RefCell<Vec<Site>>,
    /// The carries held, one per pointer, in the order they were lifted.
    placing: RefCell<Vec<sites::Placing>>,
    /// The item a lift is lifting, while the sites give their places for it.
    lifting: Cell<Option<Rect>>,
    /// Each carry's payload, by the id its lift returned.
    lifted: OwnedSignal<Vec<(CarryId, Erased)>>,
    /// The frame each carry shows.
    shown: OwnedSignal<Vec<(CarryId, DragFrame)>>,
    /// What the last carry said for a screen reader, and which carry said it.
    said: OwnedSignal<Option<(CarryId, String)>>,
}

impl Board {
    pub(crate) fn new(space: Rc<dyn Space>) -> Self {
        Self {
            space,
            orders: RefCell::default(),
            outside: Cell::default(),
            scrollers: RefCell::default(),
            scrolling: RefCell::default(),
            next: Cell::default(),
            sites: RefCell::default(),
            placing: RefCell::default(),
            lifting: Cell::default(),
            lifted: OwnedSignal::new(Vec::new()),
            shown: OwnedSignal::new(Vec::new()),
            said: OwnedSignal::new(None),
        }
    }

    /// Notes the paint order and window of every node a carry's places and the scrollers
    /// are relative to, as the core knows them now.
    fn note_order(&self, order: Order) {
        if !self.carrying() {
            return;
        }
        let nodes: Vec<NodeId> = self
            .scrollers
            .borrow()
            .iter()
            .map(|scroller| scroller.node)
            .chain(self.placing_nodes())
            .filter_map(|node| node.try_get())
            .collect();
        *self.orders.borrow_mut() = nodes.into_iter().map(|node| (node, order(node))).collect();
    }

    fn window(&self, node: NodeId) -> WindowId {
        self.known_window(node).unwrap_or(WindowId::MAIN)
    }

    /// The window `node` was in at the last point the core tracked, if it was noted.
    fn known_window(&self, node: NodeId) -> Option<WindowId> {
        self.orders.borrow().get(&node).map(|(_, window)| *window)
    }

    fn painted(&self, node: NodeId) -> Option<Vec<usize>> {
        self.orders
            .borrow()
            .get(&node)
            .map(|(order, _)| order.clone())
    }

    /// Whether anything is carried.
    pub fn carrying(&self) -> bool {
        self.placing()
    }

    pub fn set_outside(&self, outside: bool) {
        self.outside.set(outside);
    }

    /// Puts down every carry, dropping nothing: Escape, or a site whose own gesture ended
    /// under the carry (a canvas whose move an undo took back). Whether anything was.
    pub fn cancel(&self) -> bool {
        self.put_down_all()
    }

    /// Chooses again where each pointer last was: what is under it moved (a scroll, a
    /// canvas's camera), though the pointer did not.
    pub fn retrack(&self) {
        for last in self.placing_last() {
            self.place(last, false);
        }
    }

    pub fn register_scroller(&self, mut scroller: EdgeScroller) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        scroller.id = id;
        self.scrollers.borrow_mut().push(scroller);
        id
    }

    pub fn unregister_scroller(&self, id: u64) {
        self.scrollers
            .borrow_mut()
            .retain(|scroller| scroller.id != id);
        let stopped = self
            .scrolling
            .borrow()
            .as_ref()
            .is_some_and(|(scrolling, ..)| *scrolling == id);
        if stopped {
            self.scrolling.borrow_mut().take();
        }
    }

    /// Scrolls the innermost scroller whose edge band holds `point`, for the carry of its
    /// pointer, unless that carry scrolls nothing or another pointer's carry is scrolling.
    fn edge_scroll(&self, point: DragPoint) {
        if !self.carrying() || self.outside.get() || !self.placing_scrolls(point.pointer) {
            self.stop_edge_of(point.pointer);
            return;
        }
        let scrolling = self
            .scrolling
            .borrow()
            .as_ref()
            .map(|(.., pointer)| *pointer);
        if scrolling.is_some_and(|other| other != point.pointer && self.carried_by(other)) {
            return;
        }
        let found = self
            .scrollers
            .borrow()
            .iter()
            .filter_map(|scroller| {
                let node = scroller.node.try_get()?;
                if self.window(node) != point.window {
                    return None;
                }
                let rect = self.space.visible(node)?;
                if !rect.contains_half_open(point.pos) {
                    return None;
                }
                let depth = scroller.depth(rect, point.pos)?;
                Some((
                    (self.painted(node), Smaller(rect.width() * rect.height())),
                    (scroller.id, depth, Rc::clone(&scroller.scroll)),
                ))
            })
            .max_by(|(order, _), (other, _)| order.cmp(other))
            .map(|(_, found)| found);
        let previous = self
            .scrolling
            .borrow()
            .as_ref()
            .map(|(id, depth, ..)| (*id, *depth));
        match (previous, found) {
            (Some((was, depth)), Some((id, now, scroll))) if was == id => {
                if depth != now {
                    *self.scrolling.borrow_mut() =
                        Some((id, now, Rc::clone(&scroll), point.pointer));
                    scroll(Some(now));
                }
            }
            (_, Some((id, depth, scroll))) => {
                self.stop_edge();
                *self.scrolling.borrow_mut() = Some((id, depth, Rc::clone(&scroll), point.pointer));
                scroll(Some(depth));
            }
            (_, None) => self.stop_edge(),
        }
    }

    /// Stops the edge scrolling `pointer`'s carry is doing, if any.
    pub(crate) fn stop_edge_of(&self, pointer: PointerId) {
        let ours = self
            .scrolling
            .borrow()
            .as_ref()
            .is_some_and(|(.., scrolling)| *scrolling == pointer);
        if ours {
            self.stop_edge();
        }
    }

    pub fn stop_edge(&self) {
        let stopped = self.scrolling.borrow_mut().take();
        if let Some((_, _, scroll, _)) = stopped {
            scroll(None);
        }
    }
}

struct Smaller(f32);

impl PartialEq for Smaller {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Smaller {}

impl PartialOrd for Smaller {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Smaller {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.total_cmp(&self.0)
    }
}

impl Driver for Board {
    fn carrying(&self) -> bool {
        Board::carrying(self)
    }

    fn set_outside(&self, outside: bool) {
        Board::set_outside(self, outside);
    }

    fn track(&self, point: DragPoint, order: Order) {
        if self.carried_by(point.pointer) {
            self.note_order(order);
            self.place(point, false);
            self.edge_scroll(point);
        }
    }

    fn release(&self, pointer: PointerId, cancelled: bool, _order: Order) {
        self.put_down(pointer, !cancelled);
    }

    fn cancel(&self) -> bool {
        Board::cancel(self)
    }

    fn stop_edge(&self) {
        Board::stop_edge(self);
    }

    fn lifts(&self, pointer: PointerId) -> bool {
        Board::lifts(self, pointer)
    }

    fn lift(&self, carry: beui_view::drag::Carry, point: DragPoint) -> Option<CarryId> {
        Board::lift(self, carry, point)
    }

    fn put_down(&self, carry: CarryId) {
        let pointer = self
            .placing
            .borrow()
            .iter()
            .find(|placing| placing.id == carry)
            .map(|placing| placing.pointer);
        if let Some(pointer) = pointer {
            Board::put_down(self, pointer, false);
        }
    }

    fn holds(&self, carry: CarryId) -> bool {
        self.lifted
            .with(|lifted| lifted.iter().any(|(id, _)| *id == carry))
    }

    fn frame(&self, carry: CarryId) -> Option<DragFrame> {
        self.frame_by(carry)
    }

    fn step(&self, toward: beui_view::types::Vec2) -> bool {
        Board::step(self, toward)
    }

    fn said(&self) -> Option<(CarryId, String)> {
        self.said.get()
    }
}

#[cfg(test)]
mod tests;
