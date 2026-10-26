use std::cell::Cell;
use std::rc::Rc;

use beui_tree::reactive::Callback;
use beui_types::{Direction, ScrollPosition};

use crate::node::NodeId;

pub struct Scroller {
    node: NodeId,
    child: NodeId,
    direction: Cell<Direction>,
    wanted: Cell<f32>,
    follows: Cell<bool>,
    last: Cell<Option<ScrollPosition>>,
    on_change: Callback<ScrollPosition>,
    #[cfg(target_arch = "wasm32")]
    observer: std::cell::RefCell<Option<web_sys::ResizeObserver>>,
}

pub fn scroller(node: NodeId, child: NodeId, on_change: Callback<ScrollPosition>) -> Rc<Scroller> {
    let scroller = Rc::new(Scroller {
        node,
        child,
        direction: Cell::new(Direction::Vertical),
        wanted: Cell::new(0.0),
        follows: Cell::new(false),
        last: Cell::new(None),
        on_change,
        #[cfg(target_arch = "wasm32")]
        observer: std::cell::RefCell::default(),
    });
    let pass = Rc::clone(&scroller);
    crate::frame::on_layout(Rc::new(move || pass.lay_out()));
    scroller
}

impl Scroller {
    pub fn set_direction(&self, direction: Direction) {
        self.direction.set(direction);
        self.last.set(None);
        crate::frame::request_frame();
    }

    pub fn set_offset(&self, offset: f32) {
        self.wanted.set(offset);
        crate::frame::request_frame();
    }

    pub fn set_follows_end(&self, follows: bool) {
        self.follows.set(follows);
        crate::frame::request_frame();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn lay_out(&self) -> bool {
        let _ = (self.node, self.child, &self.on_change);
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn lay_out(&self) -> bool {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        let Some(element) =
            crate::dom::dom(self.node).and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
        else {
            return false;
        };
        if !element.is_connected() {
            return false;
        }
        if self.observer.borrow().is_none() {
            let callback = Closure::<dyn FnMut()>::new(crate::frame::request_frame);
            if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
                observer.observe(&element);
                if let Some(child) = crate::dom::dom(self.child)
                    .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
                {
                    observer.observe(&child);
                }
                *self.observer.borrow_mut() = Some(observer);
            }
            callback.forget();
        }
        let horizontal = self.direction.get() == Direction::Horizontal;
        let (content, viewport) = match horizontal {
            true => (element.scroll_width(), element.client_width()),
            false => (element.scroll_height(), element.client_height()),
        };
        let (content, viewport) = (content as f32, viewport as f32);
        if self.follows.get() {
            self.wanted.set((content - viewport).max(0.0));
        }
        let wanted = self.wanted.get();
        let shown = match horizontal {
            true => element.scroll_left() as f32,
            false => element.scroll_top() as f32,
        };
        if (shown - wanted.clamp(0.0, (content - viewport).max(0.0))).abs() >= 0.5 {
            match horizontal {
                true => element.set_scroll_left(wanted.round() as i32),
                false => element.set_scroll_top(wanted.round() as i32),
            }
        }
        let position = ScrollPosition {
            offset: wanted,
            content,
            viewport,
        };
        if self.last.replace(Some(position)) == Some(position) {
            return false;
        }
        self.on_change.call(position);
        true
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Scroller {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.get_mut().take() {
            observer.disconnect();
        }
    }
}
