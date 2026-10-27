use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui_tree::reactive::Callback;
use beui_types::{CanvasView, Pos2, Vec2};

use crate::css::px;
use crate::document::set_style;
use crate::node::NodeId;

thread_local! {
    static CANVASES: RefCell<HashMap<NodeId, Weak<Canvas>>> = RefCell::default();
    static ITEMS: RefCell<HashMap<NodeId, Rc<Item>>> = RefCell::default();
}

pub struct Canvas {
    canvas: NodeId,
    view_node: NodeId,
    view: Cell<Option<CanvasView>>,
    placed_at: Cell<Pos2>,
    unclipped: Cell<usize>,
    on_reveal: Callback<Vec2>,
    revealing: Cell<Option<NodeId>>,
}

struct Item {
    clip: Cell<bool>,
    canvas: RefCell<Weak<Canvas>>,
}

pub fn canvas(canvas: NodeId, view_node: NodeId, on_reveal: Callback<Vec2>) -> Rc<Canvas> {
    let state = Rc::new(Canvas {
        canvas,
        view_node,
        view: Cell::new(None),
        placed_at: Cell::new(Pos2::ZERO),
        unclipped: Cell::new(0),
        on_reveal,
        revealing: Cell::new(None),
    });
    CANVASES.with(|held| {
        let mut held = held.borrow_mut();
        held.retain(|_, canvas| canvas.strong_count() > 0);
        held.insert(canvas, Rc::downgrade(&state));
        held.insert(view_node, Rc::downgrade(&state));
    });
    state.apply();
    let pass = Rc::clone(&state);
    crate::frame::on_layout(Rc::new(move || pass.lay_out()));
    state
}

fn find(node: NodeId) -> Option<Rc<Canvas>> {
    CANVASES.with(|held| held.borrow().get(&node).and_then(Weak::upgrade))
}

pub fn reveal_in(viewer: NodeId, node: NodeId) {
    if let Some(canvas) = find(viewer).filter(|canvas| canvas.canvas == viewer) {
        canvas.revealing.set(Some(node));
    }
}

pub fn scale_property(length: f32) -> String {
    format!("calc({} * var(--beui-scale, 1))", px(length))
}

impl Canvas {
    pub fn set_view(&self, view: Option<CanvasView>) {
        if self.view.replace(view) != view {
            self.apply();
            crate::frame::request_frame();
        }
    }

    fn scale(&self) -> f32 {
        self.view
            .get()
            .map(|view| view.scale)
            .filter(|scale| *scale > 0.0 && scale.is_finite())
            .unwrap_or(1.0)
    }

    fn shift(&self) -> Vec2 {
        self.view
            .get()
            .map_or(Vec2::ZERO, |view| view.origin - self.placed_at.get())
    }

    fn apply(&self) {
        let shift = self.shift();
        set_style(
            self.view_node,
            "transform",
            (shift != Vec2::ZERO).then(|| format!("translate({}, {})", px(shift.x), px(shift.y))),
        );
        let scale = self.scale();
        set_style(
            self.view_node,
            "--beui-scale",
            (scale != 1.0).then(|| crate::css::number(scale)),
        );
    }

    fn count_unclipped(&self, change: isize) {
        let count = self.unclipped.get().saturating_add_signed(change);
        self.unclipped.set(count);
        set_style(
            self.canvas,
            "overflow",
            Some(if count == 0 { "hidden" } else { "visible" }.to_owned()),
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn lay_out(&self) -> bool {
        let _ = (&self.on_reveal, self.revealing.get());
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn lay_out(&self) -> bool {
        let Some(element) = crate::dom::element(self.canvas) else {
            return false;
        };
        if !element.is_connected() {
            return false;
        }
        let rect = crate::dom::client_rect(&element);
        let origin = crate::dom::origin();
        let placed_at = Pos2::new(rect.min.x - origin.x, rect.min.y - origin.y);
        let mut changed = false;
        if self.view.get().is_some() && (placed_at - self.placed_at.get()).length() >= 0.25 {
            self.placed_at.set(placed_at);
            self.apply();
            changed = true;
        }
        if self.on_reveal.is_empty() {
            self.revealing.set(None);
            return changed;
        }
        let scrolled = Vec2::new(element.scroll_left() as f32, element.scroll_top() as f32);
        if scrolled != Vec2::ZERO {
            element.set_scroll_left(0);
            element.set_scroll_top(0);
            self.on_reveal.call(-scrolled);
            changed = true;
        }
        if let Some(target) = self.revealing.take()
            && let Some(target) = crate::dom::element(target)
            && element.contains(Some(target.as_ref()))
        {
            let wanted = crate::dom::client_rect(&target);
            let delta = Vec2::new(
                revealing(wanted.min.x, wanted.max.x, rect.min.x, rect.max.x),
                revealing(wanted.min.y, wanted.max.y, rect.min.y, rect.max.y),
            );
            if delta.length() >= 0.5 {
                self.on_reveal.call(delta);
                changed = true;
            }
        }
        changed
    }
}

#[cfg(target_arch = "wasm32")]
fn revealing(start: f32, end: f32, shown_start: f32, shown_end: f32) -> f32 {
    if end - start > shown_end - shown_start || start < shown_start {
        shown_start - start
    } else if end > shown_end {
        shown_end - end
    } else {
        0.0
    }
}

pub fn item(node: NodeId) {
    ITEMS.with(|held| {
        held.borrow_mut().insert(
            node,
            Rc::new(Item {
                clip: Cell::new(true),
                canvas: RefCell::default(),
            }),
        )
    });
    reactive::on_cleanup(move || {
        ITEMS.with(|held| held.borrow_mut().remove(&node));
    });
}

fn item_state(node: NodeId) -> Option<Rc<Item>> {
    ITEMS.with(|held| held.borrow().get(&node).cloned())
}

pub fn set_item_clip(node: NodeId, clip: bool) {
    let Some(item) = item_state(node) else {
        return;
    };
    if item.clip.replace(clip) == clip {
        return;
    }
    if let Some(canvas) = item.canvas.borrow().upgrade() {
        canvas.count_unclipped(if clip { -1 } else { 1 });
    }
}

pub(crate) fn place(view_node: NodeId, node: NodeId) {
    let (Some(canvas), Some(item)) = (find(view_node), item_state(node)) else {
        return;
    };
    let previous = item.canvas.replace(Rc::downgrade(&canvas)).upgrade();
    if let Some(previous) = previous
        && !item.clip.get()
    {
        previous.count_unclipped(-1);
    }
    if !item.clip.get() {
        canvas.count_unclipped(1);
    }
    reactive::on_cleanup(move || {
        let held = item.canvas.replace(Weak::new()).upgrade();
        if let Some(held) = held
            && !item.clip.get()
        {
            held.count_unclipped(-1);
        }
    });
}
