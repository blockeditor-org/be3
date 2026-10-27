pub use beui_types::drag::DragPoint;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_types::{Rect, WindowId};

pub use beui_types::drag::{
    Accepts, DragSource, Dropped, EDGE_BAND, EdgeScroller, Outside, Over, Pending, Target,
};

struct Carried {
    source: DragSource,
    over: Option<u64>,
    last: Option<DragPoint>,
    floating: Option<bool>,
}

#[derive(Default)]
pub struct Board {
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    outside: Cell<bool>,
    scrollers: RefCell<Vec<EdgeScroller>>,
    scrolling: RefCell<Option<(u64, f32, Rc<dyn Fn(Option<f32>)>)>>,
    next: Cell<u64>,
    #[cfg(target_arch = "wasm32")]
    following: RefCell<Vec<crate::dom::Listener>>,
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
        #[cfg(target_arch = "wasm32")]
        {
            *self.following.borrow_mut() = web::follow();
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
        if self.outside.get() {
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

    pub fn under(&self, point: DragPoint) -> Option<(u64, (Rect, f32), Over, bool)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.source.payload.as_ref();
        if point.window != WindowId::MAIN {
            return None;
        }
        self.targets
            .borrow()
            .iter()
            .filter_map(|target| Some((self.rect_of(target)?, target)))
            .filter(|((rect, _), _)| rect.contains_half_open(point.pos))
            .filter(|(rect, target)| {
                (target.accepts)(payload) && (target.takes)(point.within(*rect))
            })
            .last()
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
        #[cfg(target_arch = "wasm32")]
        drop(std::mem::take(&mut *self.following.borrow_mut()));
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
        if !self.carrying() || self.outside.get() || point.window != WindowId::MAIN {
            self.stop_edge();
            return;
        }
        let found = self
            .scrollers
            .borrow()
            .iter()
            .filter_map(|scroller| {
                let rect = crate::measure::rect(scroller.node.try_get()?)?;
                if !rect.contains_half_open(point.pos) {
                    return None;
                }
                let depth = scroller.depth(rect, point.pos)?;
                Some((
                    rect.width() * rect.height(),
                    scroller.id,
                    depth,
                    Rc::clone(&scroller.scroll),
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id, depth, scroll)| (id, depth, scroll));
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

pub fn drag_board() -> Rc<Board> {
    crate::document::extension_or_insert_with(|| {
        crate::keys::listen(
            crate::keys::KeyAt::Window,
            crate::keys::KeyPhase::Capture,
            Rc::new(|event: crate::keys::KeyEvent| match event.input {
                crate::keys::KeyInput::Press(press) => {
                    press.pressed && press.key == beui_types::Key::Escape && drag_board().cancel()
                }
                crate::keys::KeyInput::Tap(_) => false,
            }),
        );
        Rc::new(Board::default())
    })
}

#[cfg(target_arch = "wasm32")]
mod web {
    use beui_types::{DragPoint, Pos2, pos2};
    use wasm_bindgen::JsCast;

    fn point(event: &web_sys::PointerEvent) -> DragPoint {
        let origin = crate::run::root()
            .and_then(crate::dom::dom)
            .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
            .map(|root| {
                let rect = root.get_bounding_client_rect();
                pos2(rect.left() as f32, rect.top() as f32)
            })
            .unwrap_or(Pos2::ZERO);
        let modifiers = beui_web_input::keys::modifiers(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        DragPoint::at(
            pos2(
                event.client_x() as f32 - origin.x,
                event.client_y() as f32 - origin.y,
            ),
            modifiers,
        )
    }

    pub(super) fn follow() -> Vec<crate::dom::Listener> {
        let Some(window) = crate::dom::window() else {
            return Vec::new();
        };
        let target: &web_sys::EventTarget = window.as_ref();
        vec![
            crate::dom::listen(
                target,
                "pointermove",
                true,
                |event: web_sys::PointerEvent| {
                    super::drag_board().track(point(&event));
                    reactive::flush();
                },
            ),
            crate::dom::listen(target, "pointerup", true, |event: web_sys::PointerEvent| {
                let board = super::drag_board();
                board.track(point(&event));
                board.finish();
                reactive::flush();
            }),
            crate::dom::listen(target, "pointercancel", true, |_: web_sys::PointerEvent| {
                super::drag_board().end(None);
                reactive::flush();
            }),
        ]
    }
}
