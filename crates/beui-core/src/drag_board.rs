pub use beui_types::drag::DragPoint;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use foldhash::HashMap;

use crate::document::Document;
use crate::geometry::Rect;
use crate::node::Rects;
use crate::window::WindowId;

pub use beui_types::drag::{
    Accepts, DragSource, Dropped, EDGE_BAND, EdgeScroller, Outside, Over, Pending, Target,
};

struct Carried {
    source: DragSource,
    over: Option<u64>,
    last: Option<DragPoint>,
    floating: Option<bool>,
}

pub struct Board {
    rects: Rc<Rects>,
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    painted: RefCell<HashMap<u64, Vec<usize>>>,
    windows: RefCell<HashMap<u64, WindowId>>,
    outside: Cell<bool>,
    scrollers: RefCell<Vec<EdgeScroller>>,
    scrolling: RefCell<Option<(u64, f32, Rc<dyn Fn(Option<f32>)>)>>,
    next: Cell<u64>,
}

impl Board {
    pub fn new(rects: Rc<Rects>) -> Self {
        Self {
            rects,
            pressed: RefCell::default(),
            carried: RefCell::default(),
            targets: RefCell::default(),
            painted: RefCell::default(),
            windows: RefCell::default(),
            outside: Cell::default(),
            scrollers: RefCell::default(),
            scrolling: RefCell::default(),
            next: Cell::default(),
        }
    }

    fn rect_of(&self, target: &Target) -> Option<(Rect, f32)> {
        self.rects.scaled(&target.node.try_get()?)
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

    pub fn order(&self, doc: &Document) {
        if !self.carrying() && self.pressed.borrow().is_none() {
            return;
        }
        let nodes = self
            .targets
            .borrow()
            .iter()
            .map(|target| (target.id, target.node))
            .chain(
                self.scrollers
                    .borrow()
                    .iter()
                    .map(|scroller| (scroller.id, scroller.node)),
            )
            .filter_map(|(id, node)| Some((id, node.try_get()?)))
            .collect::<Vec<_>>();
        *self.painted.borrow_mut() = nodes
            .iter()
            .map(|(id, node)| (*id, doc.paint_order(*node)))
            .collect();
        *self.windows.borrow_mut() = nodes
            .iter()
            .map(|(id, node)| (*id, doc.window_of(*node)))
            .collect();
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

    pub fn finish(&self) {
        let last = self
            .carried
            .borrow()
            .as_ref()
            .and_then(|carried| carried.last);
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
            beui_types::drag::holder(&self.targets.borrow(), carried.source.payload.as_ref())
                .is_some()
        })
    }

    pub fn under(&self, point: DragPoint) -> Option<(u64, (Rect, f32), Over, bool)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.source.payload.as_ref();
        let windows = self.windows.borrow();
        let found = self
            .targets
            .borrow()
            .iter()
            .filter_map(|target| Some((self.rect_of(target)?, target)))
            .filter(|(_, target)| {
                windows.get(&target.id).copied().unwrap_or(WindowId::MAIN) == point.window
            })
            .filter(|(_, target)| {
                target
                    .node
                    .try_get()
                    .and_then(|node| self.rects.visible(&node))
                    .is_some_and(|visible| visible.contains_half_open(point.pos))
            })
            .filter(|(rect, target)| {
                (target.accepts)(payload) && (target.takes)(point.within(*rect))
            })
            .max_by(|(_, target), (_, other)| {
                let painted = self.painted.borrow();
                painted.get(&target.id).cmp(&painted.get(&other.id))
            })
            .map(|(rect, target)| {
                (
                    target.id,
                    rect,
                    Rc::clone(&target.over),
                    (target.hides_preview)(),
                )
            });
        if found.is_some() {
            return found;
        }
        let targets = self.targets.borrow();
        let holder = beui_types::drag::holder(&targets, payload)?;
        let target = targets.iter().find(|target| target.id == holder)?;
        let rect = self.rect_of(target)?;
        (target.takes)(point.within(rect)).then(|| {
            (
                target.id,
                rect,
                Rc::clone(&target.over),
                (target.hides_preview)(),
            )
        })
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
        let point = self.stamp(point);
        let under = self.under(point);
        let id = under.as_ref().map(|(id, ..)| *id);
        let previous = self
            .carried
            .borrow_mut()
            .as_mut()
            .and_then(|carried| std::mem::replace(&mut carried.over, id));
        if previous.is_some()
            && previous != id
            && let Some(left) = previous.and_then(|id| self.over(id))
        {
            left(None);
        }
        self.float(!under.as_ref().is_some_and(|(.., hides)| *hides));
        let payload = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.source.payload));
        if let (Some((_, rect, entered, _)), Some(payload)) = (under, payload) {
            entered(Some((payload.as_ref(), point.within(rect))));
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
            if carried.over == Some(id) {
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
        self.painted.borrow_mut().remove(&id);
        self.windows.borrow_mut().remove(&id);
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
        let windows = self.windows.borrow();
        let painted = self.painted.borrow();
        let found = self
            .scrollers
            .borrow()
            .iter()
            .filter(|scroller| {
                windows.get(&scroller.id).copied().unwrap_or(WindowId::MAIN) == point.window
            })
            .filter_map(|scroller| {
                let rect = self.rects.visible(&scroller.node.try_get()?)?;
                if !rect.contains_half_open(point.pos) {
                    return None;
                }
                let depth = scroller.depth(rect, point.pos)?;
                Some((scroller.id, depth, Rc::clone(&scroller.scroll)))
            })
            .max_by(|(id, ..), (other, ..)| painted.get(id).cmp(&painted.get(other)));
        drop((windows, painted));
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

impl Document {
    pub fn drag_board(&self) -> Rc<Board> {
        Rc::clone(&self.drags)
    }

    pub fn dragging(&self) -> bool {
        self.drags.carrying()
    }
}
