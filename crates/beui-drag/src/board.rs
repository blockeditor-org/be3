use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_types::Rect;
use beui_types::node::NodeId;

use crate::{DragPoint, DragSource, EdgeScroller, Over, Pending, Surface, Target};

struct Carried {
    source: DragSource,
    over: Option<u64>,
    last: Option<DragPoint>,
    floating: Option<bool>,
}

type Scrolling = (u64, f32, Rc<dyn Fn(Option<f32>)>);

type Under = (u64, (Rect, f32), Over, bool);

pub struct Board<S> {
    surface: S,
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    outside: Cell<bool>,
    scrollers: RefCell<Vec<EdgeScroller>>,
    scrolling: RefCell<Option<Scrolling>>,
    next: Cell<u64>,
}

impl<S: Surface> Board<S> {
    pub fn new(surface: S) -> Self {
        Self {
            surface,
            pressed: RefCell::default(),
            carried: RefCell::default(),
            targets: RefCell::default(),
            outside: Cell::default(),
            scrollers: RefCell::default(),
            scrolling: RefCell::default(),
            next: Cell::default(),
        }
    }

    pub fn surface(&self) -> &S {
        &self.surface
    }

    pub fn nodes(&self) -> Vec<NodeId> {
        if !self.carrying() && self.pressed.borrow().is_none() {
            return Vec::new();
        }
        self.targets
            .borrow()
            .iter()
            .map(|target| target.node)
            .chain(self.scrollers.borrow().iter().map(|scroller| scroller.node))
            .filter_map(|node| node.try_get())
            .collect()
    }

    fn rect_of(&self, target: &Target) -> Option<(Rect, f32)> {
        self.surface.placed(target.node.try_get()?)
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
            last: None,
            floating: None,
        });
        for mark in marks {
            mark(true);
        }
        self.surface.carrying(true);
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
        let under = |target: &Target, rect| {
            (
                target.id,
                rect,
                Rc::clone(&target.over),
                (target.hides_preview)(),
            )
        };
        let targets = self.targets.borrow();
        let found = targets
            .iter()
            .filter_map(|target| Some((target.node.try_get()?, target)))
            .filter(|(node, _)| self.surface.window(*node) == point.window)
            .filter(|(node, _)| {
                self.surface
                    .visible(*node)
                    .is_some_and(|visible| visible.contains_half_open(point.pos))
            })
            .filter_map(|(node, target)| Some((node, self.rect_of(target)?, target)))
            .filter(|(_, rect, target)| {
                (target.accepts)(payload) && (target.takes)(point.within(*rect))
            })
            .max_by_key(|(node, ..)| self.surface.target_order(*node))
            .map(|(_, rect, target)| under(target, rect));
        if found.is_some() {
            return found;
        }
        let holder = holder(&targets, payload)?;
        let target = targets.iter().find(|target| target.id == holder)?;
        let rect = self.rect_of(target)?;
        (target.takes)(point.within(rect)).then(|| under(target, rect))
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
        self.surface.carrying(false);
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
                if self.surface.window(node) != point.window {
                    return None;
                }
                let rect = self.surface.visible(node)?;
                if !rect.contains_half_open(point.pos) {
                    return None;
                }
                let depth = scroller.depth(rect, point.pos)?;
                Some((
                    self.surface.scroller_order(node, rect),
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
