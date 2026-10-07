use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ::reactive::ReadSignal;

use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::input::{Modifiers, PointerPress};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragPoint {
    pub pos: Pos2,
    pub modifiers: Modifiers,
}

impl DragPoint {
    pub fn of(press: PointerPress) -> Self {
        Self {
            pos: press.pos,
            modifiers: press.modifiers,
        }
    }
}

pub type Accepts = Rc<dyn Fn(&dyn Any) -> bool>;
pub type Dropped = Rc<dyn Fn(&dyn Any, DragPoint)>;
pub type Over = Rc<dyn Fn(Option<(&dyn Any, DragPoint)>)>;
pub type Pending = Rc<dyn Fn(DragPoint)>;

pub struct Target {
    pub id: u64,
    pub depth: usize,
    pub rect: ReadSignal<Rect>,
    pub accepts: Accepts,
    pub carrying: Rc<dyn Fn(bool)>,
    pub over: Over,
    pub dropped: Dropped,
}

pub struct Carried {
    pub payload: Rc<dyn Any>,
    pub over: Option<u64>,
    pub last: Option<DragPoint>,
    pub follow: Rc<dyn Fn(Pos2)>,
}

#[derive(Default)]
pub struct Board {
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    next: Cell<u64>,
    still: Cell<bool>,
}

impl Board {
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

    pub fn set_still(&self, still: bool) {
        self.still.set(still);
    }

    pub fn carrying(&self) -> bool {
        self.carried.borrow().is_some()
    }

    pub fn begin(&self, payload: Rc<dyn Any>, point: DragPoint, follow: Rc<dyn Fn(Pos2)>) {
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

    pub fn track(&self, point: DragPoint) {
        if !self.carrying() {
            let pending = self.pressed.borrow().clone();
            if let Some(pending) = pending {
                pending(point);
            }
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
            if self.still.get() {
                return;
            }
            Rc::clone(&carried.follow)
        };
        follow(point.pos);
        self.move_to(point);
    }

    pub fn finish(&self) {
        let last = self
            .carried
            .borrow()
            .as_ref()
            .and_then(|carried| carried.last);
        self.end(last);
    }

    pub fn under(&self, pos: Pos2) -> Option<(u64, Over)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.payload.as_ref();
        self.targets
            .borrow()
            .iter()
            .filter(|target| target.rect.get_untracked().contains_half_open(pos))
            .filter(|target| (target.accepts)(payload))
            .max_by_key(|target| (target.depth, target.id))
            .map(|target| (target.id, Rc::clone(&target.over)))
    }

    pub fn over(&self, id: u64) -> Option<Over> {
        self.targets
            .borrow()
            .iter()
            .find(|target| target.id == id)
            .map(|target| Rc::clone(&target.over))
    }

    pub fn move_to(&self, point: DragPoint) {
        if !self.carrying() {
            return;
        }
        let under = self.under(point.pos);
        let previous = self.carried.borrow_mut().as_mut().and_then(|carried| {
            std::mem::replace(&mut carried.over, under.as_ref().map(|(id, _)| *id))
        });
        if previous.is_some()
            && previous != under.as_ref().map(|(id, _)| *id)
            && let Some(left) = previous.and_then(|id| self.over(id))
        {
            left(None);
        }
        let payload = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.payload));
        if let (Some((_, entered)), Some(payload)) = (under, payload) {
            entered(Some((payload.as_ref(), point)));
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
                drop = Some(dropped);
            }
            over(None);
            carrying(false);
        }
        if let (Some(dropped), Some(point)) = (drop, point) {
            dropped(carried.payload.as_ref(), point);
        }
    }
}

impl Document {
    pub fn drag_board(&self) -> Rc<Board> {
        Rc::clone(&self.drags)
    }

    pub fn dragging(&self) -> bool {
        self.drags.carrying()
    }
}
