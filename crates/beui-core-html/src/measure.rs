use stable_hash::HashMap;
use std::cell::RefCell;

use beui_types::text::LineBox;
use beui_types::{Measured, Pos2, Rect};
use reactive::{Get, OwnedSignal, ReadSignal, Set, Signal};

use crate::node::NodeId;

#[derive(Clone, PartialEq, Debug)]
pub struct TextGeometry {
    bounds: Rect,
    node: NodeId,
    revision: u64,
}

impl TextGeometry {
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn scale(&self) -> f32 {
        1.0
    }

    pub fn caret_rect(&self, index: usize, width: f32) -> Option<Rect> {
        #[cfg(target_arch = "wasm32")]
        return crate::text::caret_rect(self.node, index, width);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (index, width);
            None
        }
    }

    pub fn index_at(&self, pos: Pos2) -> Option<usize> {
        #[cfg(target_arch = "wasm32")]
        return crate::text::index_at(self.node, pos);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = pos;
            None
        }
    }

    pub fn inline_at(&self, pos: Pos2) -> Option<usize> {
        #[cfg(target_arch = "wasm32")]
        return crate::text::inline_at(self.node, pos);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = pos;
            None
        }
    }

    pub fn lines(&self) -> Vec<LineBox> {
        #[cfg(target_arch = "wasm32")]
        return crate::text::lines(self.node);
        #[cfg(not(target_arch = "wasm32"))]
        Vec::new()
    }
}

thread_local! {
    static RECTS: RefCell<HashMap<NodeId, OwnedSignal<Option<Measured>>>> = RefCell::default();
    static TEXTS: RefCell<HashMap<NodeId, OwnedSignal<Option<TextGeometry>>>> = RefCell::default();
}

fn watch<T: 'static>(
    store: &'static std::thread::LocalKey<RefCell<HashMap<NodeId, OwnedSignal<Option<T>>>>>,
    node: NodeId,
) -> ReadSignal<Option<T>> {
    let held = store.with(|held| held.borrow().get(&node).map(|signal| signal.handle()));
    let signal = match held {
        Some(signal) => signal,
        None => {
            let owned = OwnedSignal::new(None);
            let signal = owned.handle();
            store.with(|held| held.borrow_mut().insert(node, owned));
            crate::frame::request_frame();
            signal
        }
    };
    signal.read_only()
}

pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    watch(&RECTS, node)
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    watch(&TEXTS, node)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watched() -> bool {
    RECTS.with(|held| !held.borrow().is_empty())
        || TEXTS.with(|held| !held.borrow().is_empty())
        || crate::text::decorated()
}

fn publish<T: Clone + PartialEq + 'static>(
    store: &'static std::thread::LocalKey<RefCell<HashMap<NodeId, OwnedSignal<Option<T>>>>>,
    read: impl Fn(NodeId) -> Option<T>,
) -> bool {
    let held: Vec<(NodeId, Signal<Option<T>>)> = store.with(|held| {
        held.borrow()
            .iter()
            .map(|(node, signal)| (*node, signal.handle()))
            .collect()
    });
    let mut changed = false;
    for (node, signal) in held {
        if !crate::document::contains(node) {
            let removed = store.with(|held| held.borrow_mut().remove(&node));
            drop(removed);
            continue;
        }
        let value = read(node);
        if signal.with_untracked(|held| *held != value) {
            signal.set(value);
            changed = true;
        }
    }
    changed
}

#[cfg(target_arch = "wasm32")]
fn text_bounds(node: NodeId, rect: Rect) -> Rect {
    crate::text::bounds(node, rect)
}

#[cfg(not(target_arch = "wasm32"))]
fn text_bounds(_node: NodeId, rect: Rect) -> Rect {
    rect
}

pub(crate) fn pass() -> bool {
    let rects = publish(&RECTS, measured);
    let texts = publish(&TEXTS, |node| {
        rect(node).map(|rect| TextGeometry {
            bounds: text_bounds(node, rect),
            node,
            revision: crate::text::revision(node),
        })
    });
    rects | texts
}

#[cfg(target_arch = "wasm32")]
fn measured(node: NodeId) -> Option<Measured> {
    use wasm_bindgen::JsCast;

    let rect = rect(node)?;
    let size = crate::dom::dom(node)
        .and_then(|dom| dom.dyn_into::<web_sys::HtmlElement>().ok())
        .map_or(rect.size(), |element| {
            beui_types::vec2(
                element.offset_width() as f32,
                element.offset_height() as f32,
            )
        });
    Some(Measured { rect, size })
}

#[cfg(not(target_arch = "wasm32"))]
fn measured(_node: NodeId) -> Option<Measured> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn rect(node: NodeId) -> Option<Rect> {
    use wasm_bindgen::JsCast;

    let element: web_sys::Element = crate::dom::dom(node)?.dyn_into().ok()?;
    if !element.is_connected() {
        return None;
    }
    let origin = crate::run::root()
        .and_then(crate::dom::dom)
        .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
        .map(|root| root.get_bounding_client_rect());
    let (left, top) = origin.map_or((0.0, 0.0), |origin| (origin.left(), origin.top()));
    let bounds = element.get_bounding_client_rect();
    Some(Rect::from_min_size(
        beui_types::pos2((bounds.left() - left) as f32, (bounds.top() - top) as f32),
        beui_types::vec2(bounds.width() as f32, bounds.height() as f32),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn rect(_node: NodeId) -> Option<Rect> {
    None
}
