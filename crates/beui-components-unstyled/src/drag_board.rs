use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_lib_stable_hash::HashMap;
use beui_view::drag::{
    DragPoint, DragSource, Driver, EdgeScroller, Order, Over, Pending, Space, Take, Target,
};
use beui_view::reactive::{NodeId, tree_local};
use beui_view::types::{Rect, WindowId};

/// The drag board of the current tree: what is carried, the targets and scrollers it can
/// pass over, floating, drops, cancels and edge scrolling. Each core says where its nodes
/// are (`Space`) and drives the board with pointer points (`Driver`).
pub fn drag_board() -> Rc<Board> {
    tree_local(|| {
        let board = Rc::new(Board::new(beui_view::drag::space()));
        beui_view::drag::drive(Rc::clone(&board) as Rc<dyn Driver>);
        board
    })
}

struct Carried {
    source: DragSource,
    over: Option<u64>,
    refused: bool,
    last: Option<DragPoint>,
    floating: Option<bool>,
}

type Scrolling = (u64, f32, Rc<dyn Fn(Option<f32>)>);

struct Under {
    id: u64,
    rect: (Rect, f32),
    over: Over,
    hides: bool,
    take: Take,
}

pub struct Board {
    space: Rc<dyn Space>,
    orders: RefCell<HashMap<NodeId, (Vec<usize>, WindowId)>>,
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    outside: Cell<bool>,
    scrollers: RefCell<Vec<EdgeScroller>>,
    scrolling: RefCell<Option<Scrolling>>,
    next: Cell<u64>,
}

impl Board {
    fn new(space: Rc<dyn Space>) -> Self {
        Self {
            space,
            orders: RefCell::default(),
            pressed: RefCell::default(),
            carried: RefCell::default(),
            targets: RefCell::default(),
            outside: Cell::default(),
            scrollers: RefCell::default(),
            scrolling: RefCell::default(),
            next: Cell::default(),
        }
    }

    fn order(&self, order: Order) {
        if !self.carrying() && self.pressed.borrow().is_none() {
            return;
        }
        self.note_order(order);
    }

    fn note_order(&self, order: Order) {
        let nodes: Vec<NodeId> = self
            .targets
            .borrow()
            .iter()
            .map(|target| target.node)
            .chain(self.scrollers.borrow().iter().map(|scroller| scroller.node))
            .filter_map(|node| node.try_get())
            .collect();
        *self.orders.borrow_mut() = nodes.into_iter().map(|node| (node, order(node))).collect();
    }

    fn window(&self, node: NodeId) -> WindowId {
        self.orders
            .borrow()
            .get(&node)
            .map_or(WindowId::MAIN, |(_, window)| *window)
    }

    fn painted(&self, node: NodeId) -> Option<Vec<usize>> {
        self.orders
            .borrow()
            .get(&node)
            .map(|(order, _)| order.clone())
    }

    fn rect_of(&self, target: &Target) -> Option<(Rect, f32)> {
        self.space.placed(target.node.try_get()?)
    }

    pub fn register(&self, mut target: Target) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        target.id = id;
        let carrying = self
            .carried
            .borrow()
            .as_ref()
            .is_some_and(|carried| (target.accepts)(carried.source.payload.as_ref()));
        let mark = Rc::clone(&target.carrying);
        self.targets.borrow_mut().push(target);
        if carrying {
            mark(true);
        }
        id
    }

    pub fn unregister(&self, id: u64) {
        self.targets.borrow_mut().retain(|target| target.id != id);
        if let Some(carried) = self.carried.borrow_mut().as_mut()
            && carried.over == Some(id)
        {
            carried.over = None;
        }
    }

    pub fn carrying(&self) -> bool {
        self.carried.borrow().is_some()
    }

    pub fn item(&self) -> Option<Rect> {
        self.carried
            .borrow()
            .as_ref()
            .map(|carried| carried.source.item)
    }

    pub fn begin(&self, source: DragSource, point: DragPoint) {
        let marks = self
            .targets
            .borrow()
            .iter()
            .filter(|target| (target.accepts)(source.payload.as_ref()))
            .map(|target| Rc::clone(&target.carrying))
            .collect::<Vec<_>>();
        *self.carried.borrow_mut() = Some(Carried {
            source,
            over: None,
            refused: false,
            last: None,
            floating: None,
        });
        for mark in marks {
            mark(true);
        }
        self.track(point);
    }

    pub fn press(&self, pending: Pending) {
        *self.pressed.borrow_mut() = Some(pending);
    }

    pub fn release(&self) {
        self.pressed.borrow_mut().take();
    }

    pub fn set_outside(&self, outside: bool) {
        self.outside.set(outside);
    }

    pub fn outside(&self) -> bool {
        self.outside.get()
    }

    fn leave(&self) {
        let previous = self
            .carried
            .borrow_mut()
            .as_mut()
            .and_then(|carried| carried.over.take());
        if let Some(left) = previous.and_then(|id| self.over(id)) {
            left(None);
        }
        self.float(true);
    }

    fn float(&self, floating: bool) {
        let changed = self.carried.borrow_mut().as_mut().and_then(|carried| {
            (carried.floating != Some(floating)).then(|| {
                carried.floating = Some(floating);
                Rc::clone(&carried.source.floating)
            })
        });
        if let Some(changed) = changed {
            changed(floating);
        }
    }

    fn stamp(&self, point: DragPoint) -> DragPoint {
        DragPoint {
            item: self.item().unwrap_or(point.item),
            ..point
        }
    }

    pub fn cancel(&self) -> bool {
        if !self.carrying() {
            return false;
        }
        self.end(None);
        true
    }

    pub fn track(&self, point: DragPoint) {
        if !self.carrying() {
            let pending = self.pressed.borrow().clone();
            if let Some(pending) = pending {
                pending(point);
            }
            return;
        }
        if self.outside.get() && !self.held() {
            if let Some(carried) = self.carried.borrow_mut().as_mut() {
                carried.last = Some(point);
            }
            self.stop_edge();
            self.leave();
            return;
        }
        let follow = {
            let mut carried = self.carried.borrow_mut();
            let Some(carried) = carried.as_mut() else {
                return;
            };
            if carried.last == Some(point) {
                None
            } else {
                carried.last = Some(point);
                Some(Rc::clone(&carried.source.follow))
            }
        };
        let moved = follow.is_some();
        if let Some(follow) = follow {
            follow(self.stamp(point));
        }
        self.edge_scroll(point);
        if moved || self.scrolling.borrow().is_some() {
            self.move_to(point);
        }
    }

    pub fn retrack(&self) {
        if let Some(last) = self.last() {
            self.track(last);
        }
    }

    fn last(&self) -> Option<DragPoint> {
        self.carried
            .borrow()
            .as_ref()
            .and_then(|carried| carried.last)
    }

    pub fn finish(&self) {
        let last = self.last();
        if !self.outside.get() || self.held() {
            self.end(last);
            return;
        }
        let outside = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.source.outside));
        let last = last.map(|last| self.stamp(last));
        self.leave();
        self.settle(None, false);
        if let (Some(outside), Some(last)) = (outside, last) {
            outside(last);
        }
    }

    pub fn held(&self) -> bool {
        let carried = self.carried.borrow();
        carried.as_ref().is_some_and(|carried| {
            holder(&self.targets.borrow(), carried.source.payload.as_ref()).is_some()
        })
    }

    fn under(&self, point: DragPoint) -> Option<Under> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.source.payload.as_ref();
        let under = |target: &Target, rect, take| Under {
            id: target.id,
            rect,
            over: Rc::clone(&target.over),
            hides: (target.hides_preview)(),
            take,
        };
        let targets = self.targets.borrow();
        let found = targets
            .iter()
            .filter_map(|target| Some((target.node.try_get()?, target)))
            .filter(|(node, _)| self.window(*node) == point.window)
            .filter(|(node, _)| {
                self.space
                    .visible(*node)
                    .is_some_and(|visible| visible.contains_half_open(point.pos))
            })
            .filter_map(|(node, target)| Some((node, self.rect_of(target)?, target)))
            .filter(|(.., target)| (target.accepts)(payload))
            .filter_map(|(node, rect, target)| {
                let take = (target.takes)(point.within(rect));
                (take != Take::Pass).then_some((node, rect, target, take))
            })
            .max_by_key(|(node, ..)| self.painted(*node))
            .map(|(_, rect, target, take)| under(target, rect, take));
        if found.is_some() {
            return found;
        }
        let holder = holder(&targets, payload)?;
        let target = targets.iter().find(|target| target.id == holder)?;
        let rect = self.rect_of(target)?;
        let take = (target.takes)(point.within(rect));
        (take != Take::Pass).then(|| under(target, rect, take))
    }

    fn over(&self, id: u64) -> Option<Over> {
        self.targets
            .borrow()
            .iter()
            .find(|target| target.id == id)
            .map(|target| Rc::clone(&target.over))
    }

    fn placed(&self, id: u64) -> Option<(Rect, f32)> {
        self.targets
            .borrow()
            .iter()
            .find(|target| target.id == id)
            .and_then(|target| self.rect_of(target))
    }

    fn move_to(&self, point: DragPoint) {
        if !self.carrying() {
            return;
        }
        let point = self.stamp(point);
        let under = self.under(point);
        let id = under.as_ref().map(|under| under.id);
        let refused = under
            .as_ref()
            .is_some_and(|under| under.take == Take::Refuse);
        let previous = self.carried.borrow_mut().as_mut().and_then(|carried| {
            carried.refused = refused;
            std::mem::replace(&mut carried.over, id)
        });
        if previous.is_some()
            && previous != id
            && let Some(left) = previous.and_then(|id| self.over(id))
        {
            left(None);
        }
        self.float(
            !under
                .as_ref()
                .is_some_and(|under| under.hides && under.take == Take::Drop),
        );
        let payload = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.source.payload));
        if let (Some(under), Some(payload)) = (under, payload) {
            (under.over)(Some((payload.as_ref(), point.within(under.rect))));
        }
    }

    pub fn end(&self, point: Option<DragPoint>) -> bool {
        self.settle(point, true)
    }

    fn settle(&self, point: Option<DragPoint>, tell: bool) -> bool {
        if let Some(point) = point {
            self.move_to(point);
        }
        let Some(carried) = self.carried.borrow_mut().take() else {
            return false;
        };
        self.stop_edge();
        let point = point.map(|point| DragPoint {
            item: carried.source.item,
            ..point
        });
        let targets = self
            .targets
            .borrow()
            .iter()
            .map(|target| {
                (
                    target.id,
                    Rc::clone(&target.carrying),
                    Rc::clone(&target.over),
                    Rc::clone(&target.dropped),
                )
            })
            .collect::<Vec<_>>();
        let mut drop = None;
        for (id, carrying, over, dropped) in targets {
            if carried.over == Some(id) && !carried.refused {
                drop = Some((dropped, self.placed(id)));
            }
            over(None);
            carrying(false);
        }
        if let (Some((dropped, rect)), Some(point)) = (drop, point) {
            let point = rect.map_or(point, |rect| point.within(rect));
            dropped(carried.source.payload.as_ref(), point);
            (carried.source.dropped)();
            return true;
        }
        if tell {
            (carried.source.cancelled)();
        }
        false
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

    fn edge_scroll(&self, point: DragPoint) {
        if !self.carrying() || self.outside.get() {
            self.stop_edge();
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
            .map(|(id, depth, _)| (*id, *depth));
        match (previous, found) {
            (Some((was, depth)), Some((id, now, scroll))) if was == id => {
                if depth != now {
                    *self.scrolling.borrow_mut() = Some((id, now, Rc::clone(&scroll)));
                    scroll(Some(now));
                }
            }
            (_, Some((id, depth, scroll))) => {
                self.stop_edge();
                *self.scrolling.borrow_mut() = Some((id, depth, Rc::clone(&scroll)));
                scroll(Some(depth));
            }
            (_, None) => self.stop_edge(),
        }
    }

    pub fn stop_edge(&self) {
        let stopped = self.scrolling.borrow_mut().take();
        if let Some((_, _, scroll)) = stopped {
            scroll(None);
        }
    }
}

fn holder(targets: &[Target], payload: &dyn Any) -> Option<u64> {
    let holder = targets.iter().find(|target| (target.holds)(payload))?;
    let elsewhere = targets
        .iter()
        .any(|target| target.id != holder.id && (target.accepts)(payload));
    (!elsewhere).then_some(holder.id)
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
        self.order(order);
        Board::track(self, point);
    }

    fn release(&self, cancelled: bool, order: Order) {
        self.order(order);
        match cancelled {
            true => {
                self.end(None);
            }
            false => self.finish(),
        }
    }

    fn cancel(&self) -> bool {
        Board::cancel(self)
    }

    fn stop_edge(&self) {
        Board::stop_edge(self);
    }
}

#[cfg(test)]
mod tests;
