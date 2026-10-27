pub use beui_types::drag::DragPoint;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_types::{Pos2, Rect, WindowId};

pub use beui_types::drag::{Accepts, Dropped, Outside, Over, Pending, Target};

pub struct Carried {
    pub payload: Rc<dyn Any>,
    pub over: Option<u64>,
    pub last: Option<DragPoint>,
    pub follow: Rc<dyn Fn(DragPoint)>,
    pub outside: Outside,
}

#[derive(Default)]
pub struct Board {
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    outside: Cell<bool>,
    next: Cell<u64>,
}

pub fn drag_board() -> Rc<Board> {
    crate::document::extension_or_insert_with(|| Rc::new(Board::default()))
}

impl Board {
    fn rect_of(&self, target: &Target) -> Option<(Rect, f32)> {
        Some((crate::measure::rect(target.node.try_get()?)?, 1.0))
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

    pub fn under(&self, window: WindowId, pos: Pos2) -> Option<(u64, (Rect, f32), Over)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.payload.as_ref();
        if window != WindowId::MAIN {
            return None;
        }
        self.targets
            .borrow()
            .iter()
            .filter_map(|target| Some((self.rect_of(target)?, target)))
            .filter(|((rect, _), _)| rect.contains_half_open(pos))
            .filter(|(_, target)| (target.accepts)(payload))
            .last()
            .map(|(rect, target)| (target.id, rect, Rc::clone(&target.over)))
    }

    pub fn over(&self, id: u64) -> Option<Over> {
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

    pub fn move_to(&self, point: DragPoint) {
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
