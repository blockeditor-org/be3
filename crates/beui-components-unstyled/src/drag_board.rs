use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_view::drag::{DragPoint, Driver, Order, Outside, Over, Pending, Space, Target};
use beui_view::reactive::tree_local;
use beui_view::types::{Pos2, Rect, WindowId};
use stable_hash::HashMap;

pub fn drag_board() -> Rc<Board> {
    tree_local(|| {
        let board = Rc::new(Board::new(beui_view::drag::space()));
        beui_view::drag::drive(Rc::clone(&board) as Rc<dyn Driver>);
        board
    })
}

struct Carried {
    payload: Rc<dyn Any>,
    over: Option<u64>,
    last: Option<DragPoint>,
    follow: Rc<dyn Fn(DragPoint)>,
    outside: Outside,
}

pub struct Board {
    space: Rc<dyn Space>,
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    painted: RefCell<HashMap<u64, Vec<usize>>>,
    windows: RefCell<HashMap<u64, WindowId>>,
    outside: Cell<bool>,
    next: Cell<u64>,
}

impl Board {
    fn new(space: Rc<dyn Space>) -> Self {
        Self {
            space,
            pressed: RefCell::default(),
            carried: RefCell::default(),
            targets: RefCell::default(),
            painted: RefCell::default(),
            windows: RefCell::default(),
            outside: Cell::default(),
            next: Cell::default(),
        }
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
            .is_some_and(|carried| (target.accepts)(carried.payload.as_ref()));
        let mark = Rc::clone(&target.carrying);
        self.targets.borrow_mut().push(target);
        if carrying {
            mark(true);
        }
        id
    }

    pub fn unregister(&self, id: u64) {
        self.targets.borrow_mut().retain(|target| target.id != id);
        self.painted.borrow_mut().remove(&id);
        self.windows.borrow_mut().remove(&id);
        if let Some(carried) = self.carried.borrow_mut().as_mut()
            && carried.over == Some(id)
        {
            carried.over = None;
        }
    }

    pub fn carrying(&self) -> bool {
        self.carried.borrow().is_some()
    }

    pub fn begin(
        &self,
        payload: Rc<dyn Any>,
        point: DragPoint,
        follow: Rc<dyn Fn(DragPoint)>,
        outside: Outside,
    ) {
        let marks = self
            .targets
            .borrow()
            .iter()
            .filter(|target| (target.accepts)(payload.as_ref()))
            .map(|target| Rc::clone(&target.carrying))
            .collect::<Vec<_>>();
        *self.carried.borrow_mut() = Some(Carried {
            payload,
            over: None,
            last: None,
            follow,
            outside,
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

    fn leave(&self) {
        let previous = self
            .carried
            .borrow_mut()
            .as_mut()
            .and_then(|carried| carried.over.take());
        if let Some(left) = previous.and_then(|id| self.over(id)) {
            left(None);
        }
    }

    fn order(&self, order: Order) {
        if !self.carrying() && self.pressed.borrow().is_none() {
            return;
        }
        let (painted, windows) = self
            .targets
            .borrow()
            .iter()
            .filter_map(|target| {
                let (painted, window) = order(target.node.try_get()?);
                Some(((target.id, painted), (target.id, window)))
            })
            .unzip();
        *self.painted.borrow_mut() = painted;
        *self.windows.borrow_mut() = windows;
    }

    pub fn track(&self, point: DragPoint) {
        if !self.carrying() {
            let pending = self.pressed.borrow().clone();
            if let Some(pending) = pending {
                pending(point);
            }
            return;
        }
        if self.outside.get() {
            if let Some(carried) = self.carried.borrow_mut().as_mut() {
                carried.last = Some(point);
            }
            self.leave();
            return;
        }
        let follow = {
            let mut carried = self.carried.borrow_mut();
            let Some(carried) = carried.as_mut() else {
                return;
            };
            if carried.last == Some(point) {
                return;
            }
            carried.last = Some(point);
            Rc::clone(&carried.follow)
        };
        follow(point);
        self.move_to(point);
    }

    pub fn finish(&self) {
        let last = self
            .carried
            .borrow()
            .as_ref()
            .and_then(|carried| carried.last);
        if !self.outside.get() {
            self.end(last);
            return;
        }
        let outside = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.outside));
        self.leave();
        self.end(None);
        if let (Some(outside), Some(last)) = (outside, last) {
            outside(last);
        }
    }

    fn under(&self, window: WindowId, pos: Pos2) -> Option<(u64, (Rect, f32), Over)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.payload.as_ref();
        let windows = self.windows.borrow();
        self.targets
            .borrow()
            .iter()
            .filter_map(|target| Some((self.rect_of(target)?, target)))
            .filter(|(_, target)| {
                windows.get(&target.id).copied().unwrap_or(WindowId::MAIN) == window
            })
            .filter(|(_, target)| {
                target
                    .node
                    .try_get()
                    .and_then(|node| self.space.visible(node))
                    .is_some_and(|visible| visible.contains_half_open(pos))
            })
            .filter(|(_, target)| (target.accepts)(payload))
            .max_by(|(_, target), (_, other)| {
                let painted = self.painted.borrow();
                painted.get(&target.id).cmp(&painted.get(&other.id))
            })
            .map(|(rect, target)| (target.id, rect, Rc::clone(&target.over)))
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
        let under = self.under(point.window, point.pos);
        let previous = self.carried.borrow_mut().as_mut().and_then(|carried| {
            std::mem::replace(&mut carried.over, under.as_ref().map(|(id, _, _)| *id))
        });
        if previous.is_some()
            && previous != under.as_ref().map(|(id, _, _)| *id)
            && let Some(left) = previous.and_then(|id| self.over(id))
        {
            left(None);
        }
        let payload = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.payload));
        if let (Some((_, rect, entered)), Some(payload)) = (under, payload) {
            entered(Some((payload.as_ref(), point.within(rect))));
        }
    }

    pub fn end(&self, point: Option<DragPoint>) {
        if let Some(point) = point {
            self.move_to(point);
        }
        let Some(carried) = self.carried.borrow_mut().take() else {
            return;
        };
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
            if carried.over == Some(id) {
                drop = Some((dropped, self.placed(id)));
            }
            over(None);
            carrying(false);
        }
        if let (Some((dropped, rect)), Some(point)) = (drop, point) {
            let point = rect.map_or(point, |rect| point.within(rect));
            dropped(carried.payload.as_ref(), point);
        }
    }
}

impl Driver for Board {
    fn carrying(&self) -> bool {
        Board::carrying(self)
    }

    fn set_outside(&self, outside: bool) {
        self.outside.set(outside);
    }

    fn track(&self, point: DragPoint, order: Order) {
        self.order(order);
        Board::track(self, point);
    }

    fn release(&self, cancelled: bool, order: Order) {
        self.order(order);
        match cancelled {
            true => self.end(None),
            false => self.finish(),
        }
    }
}
