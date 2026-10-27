use std::cell::RefCell;
use std::collections::HashMap;

use beui_types::text::FontId;
use beui_types::text::LineBox;
use beui_types::{Pos2, Rect, Vec2};
use reactive::{Get, ReadSignal, Set, Signal, create_signal};

use crate::node::NodeId;

#[derive(Clone, PartialEq, Debug)]
pub struct TextGeometry {
    bounds: Rect,
}

impl TextGeometry {
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn scale(&self) -> f32 {
        1.0
    }

    pub fn caret_rect(&self, _index: usize, _width: f32) -> Option<Rect> {
        None
    }

    pub fn index_at(&self, _pos: Pos2) -> Option<usize> {
        None
    }

    pub fn inline_at(&self, _pos: Pos2) -> Option<usize> {
        None
    }

    pub fn lines(&self) -> Vec<LineBox> {
        Vec::new()
    }
}

thread_local! {
    static RECTS: RefCell<HashMap<NodeId, Signal<Option<Rect>>>> = RefCell::default();
    static TEXTS: RefCell<HashMap<NodeId, Signal<Option<TextGeometry>>>> = RefCell::default();
}

fn watch<T: 'static>(
    store: &'static std::thread::LocalKey<RefCell<HashMap<NodeId, Signal<Option<T>>>>>,
    node: NodeId,
) -> ReadSignal<Option<T>> {
    let held = store.with(|held| held.borrow().get(&node).copied());
    let signal = match held {
        Some(signal) => signal,
        None => {
            let signal = match crate::document::owner(node) {
                Some(owner) => owner.run(|| create_signal(None)),
                None => create_signal(None),
            };
            store.with(|held| held.borrow_mut().insert(node, signal));
            crate::frame::request_frame();
            signal
        }
    };
    signal.read_only()
}

pub fn measure(node: NodeId) -> ReadSignal<Option<Rect>> {
    watch(&RECTS, node)
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    watch(&TEXTS, node)
}

pub fn measure_line(_text: &str, _font: FontId) -> Option<Vec2> {
    None
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watched() -> bool {
    RECTS.with(|held| !held.borrow().is_empty()) || TEXTS.with(|held| !held.borrow().is_empty())
}

fn publish<T: Clone + PartialEq + 'static>(
    store: &'static std::thread::LocalKey<RefCell<HashMap<NodeId, Signal<Option<T>>>>>,
    read: impl Fn(NodeId) -> Option<T>,
) -> bool {
    let held: Vec<(NodeId, Signal<Option<T>>)> = store.with(|held| {
        held.borrow()
            .iter()
            .map(|(node, signal)| (*node, *signal))
            .collect()
    });
    let mut changed = false;
    for (node, signal) in held {
        if !crate::document::contains(node) || !signal.is_alive() {
            store.with(|held| held.borrow_mut().remove(&node));
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

pub(crate) fn pass() -> bool {
    let rects = publish(&RECTS, rect);
    let texts = publish(&TEXTS, |node| {
        rect(node).map(|bounds| TextGeometry { bounds })
    });
    rects | texts
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
