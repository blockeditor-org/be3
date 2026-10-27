use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use foldhash::HashMap;

use crate::base::list::Direction;
use crate::callback::{Callback, NodeRef};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{AutoscrollGesture, Modifiers, PointerPress};
use crate::node::{NodeId, Rects};
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
        let press = crate::base::interactive::press_at(
            rect,
            scale,
            self.pos,
            0,
            self.modifiers,
            false,
            crate::timer::now(),
        );
        Self {
            local: press.local,
            size: press.size,
            fraction: press.fraction,
            ..self
        }
    }
}

pub type Accepts = Rc<dyn Fn(&dyn Any) -> bool>;
pub type Dropped = Rc<dyn Fn(&dyn Any, DragPoint)>;
pub type Over = Rc<dyn Fn(Option<(&dyn Any, DragPoint)>)>;
pub type Pending = Rc<dyn Fn(DragPoint)>;
pub type Outside = Rc<dyn Fn(DragPoint)>;

pub struct Target {
    pub id: u64,
    pub node: NodeRef,
    pub accepts: Accepts,
    pub carrying: Rc<dyn Fn(bool)>,
    pub over: Over,
    pub dropped: Dropped,
    pub hides_preview: Rc<dyn Fn() -> bool>,
}

pub struct DragSource {
    pub payload: Rc<dyn Any>,
    pub item: Rect,
    pub follow: Rc<dyn Fn(DragPoint)>,
    pub floating: Rc<dyn Fn(bool)>,
    pub outside: Outside,
    pub cancelled: Rc<dyn Fn()>,
}

impl DragSource {
    pub fn new(payload: Rc<dyn Any>) -> Self {
        Self {
            payload,
            item: Rect::ZERO,
            follow: Rc::new(|_| {}),
            floating: Rc::new(|_| {}),
            outside: Rc::new(|_| {}),
            cancelled: Rc::new(|| {}),
        }
    }
}

struct Carried {
    source: DragSource,
    over: Option<u64>,
    last: Option<DragPoint>,
    floating: bool,
}

#[derive(Clone, Copy)]
struct Edge {
    catcher: NodeId,
    report: Callback<AutoscrollGesture>,
    gesture: AutoscrollGesture,
}

pub const EDGE_BAND: f32 = 40.0;
const EDGE_GAIN: f32 = 3.0;

pub struct Board {
    rects: Rc<Rects>,
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    painted: RefCell<HashMap<u64, Vec<usize>>>,
    windows: RefCell<HashMap<u64, WindowId>>,
    outside: Cell<bool>,
    edge: Cell<Option<Edge>>,
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
            edge: Cell::default(),
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
            floating: true,
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
            (carried.floating != floating).then(|| {
                carried.floating = floating;
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
        self.leave();
        self.end(None);
        true
    }

    pub fn order(&self, doc: &Document) {
        if !self.carrying() && self.pressed.borrow().is_none() {
            return;
        }
        let painted = self
            .targets
            .borrow()
            .iter()
            .filter_map(|target| Some((target.id, doc.paint_order(target.node.try_get()?))))
            .collect();
        *self.painted.borrow_mut() = painted;
        let windows = self
            .targets
            .borrow()
            .iter()
            .filter_map(|target| Some((target.id, doc.window_of(target.node.try_get()?))))
            .collect();
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
                None
            } else {
                carried.last = Some(point);
                Some(Rc::clone(&carried.source.follow))
            }
        };
        match follow {
            Some(follow) => follow(self.stamp(point)),
            None if self.edge.get().is_none() => return,
            None => {}
        }
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
            .map(|carried| Rc::clone(&carried.source.outside));
        let last = last.map(|last| self.stamp(last));
        self.leave();
        self.settle(None, false);
        if let (Some(outside), Some(last)) = (outside, last) {
            outside(last);
        }
    }

    pub fn under(&self, window: WindowId, pos: Pos2) -> Option<(u64, (Rect, f32), Over, bool)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.source.payload.as_ref();
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
                    .and_then(|node| self.rects.visible(&node))
                    .is_some_and(|visible| visible.contains_half_open(pos))
            })
            .filter(|(_, target)| (target.accepts)(payload))
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
        let under = self.under(point.window, point.pos);
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
            return true;
        }
        if tell {
            (carried.source.cancelled)();
        }
        false
    }

    pub(crate) fn edge_scroll(
        &self,
        pos: Pos2,
        edges: impl Iterator<
            Item = (
                NodeId,
                Rect,
                Direction,
                Callback<AutoscrollGesture>,
                [bool; 2],
            ),
        >,
    ) {
        if !self.carrying() || self.outside.get() {
            self.stop_edge();
            return;
        }
        let found = edges
            .filter_map(|(catcher, rect, axis, report, movable)| {
                let along = |pos: Pos2| axis.main(pos.to_vec2());
                let (start, end) = (along(rect.min), along(rect.max));
                let band = EDGE_BAND.min((end - start) / 4.0);
                let at = along(pos);
                let depth = match () {
                    _ if at < start + band && movable[0] => at - (start + band),
                    _ if at > end - band && movable[1] => at - (end - band),
                    _ => return None,
                };
                let reach =
                    crate::interact::autoscroll::AUTOSCROLL_DEAD_ZONE + depth.abs() * EDGE_GAIN;
                let origin = pos - axis.axes(reach * depth.signum(), 0.0);
                Some(Edge {
                    catcher,
                    report,
                    gesture: AutoscrollGesture {
                        origin,
                        pos,
                        ended: false,
                        edge: true,
                    },
                })
            })
            .next();
        let previous = self.edge.get();
        match (previous, found) {
            (Some(previous), Some(found)) if previous.catcher == found.catcher => {
                if previous.gesture != found.gesture {
                    self.edge.set(Some(found));
                    found.report.call(found.gesture);
                }
            }
            (_, Some(found)) => {
                self.stop_edge();
                self.edge.set(Some(found));
                found.report.call(found.gesture);
            }
            (_, None) => self.stop_edge(),
        }
    }

    fn stop_edge(&self) {
        if let Some(edge) = self.edge.take() {
            edge.report.call(AutoscrollGesture {
                ended: true,
                ..edge.gesture
            });
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
