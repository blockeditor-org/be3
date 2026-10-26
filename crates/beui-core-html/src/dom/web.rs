use std::cell::RefCell;
use std::time::Duration;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::node::NodeId;

pub fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

pub fn document() -> Option<web_sys::Document> {
    window()?.document()
}

pub fn dom(node: NodeId) -> Option<web_sys::Node> {
    crate::document::element(node)?.dom()
}

pub fn html_element(node: NodeId) -> Option<web_sys::HtmlElement> {
    dom(node)?.dyn_into().ok()
}

pub fn connected(node: NodeId) -> bool {
    dom(node).is_some_and(|dom| dom.is_connected())
}

thread_local! {
    static KNOWN: std::cell::RefCell<js_sys::WeakMap> = std::cell::RefCell::new(js_sys::WeakMap::new());
}

pub fn forget_known() {
    KNOWN.with(|known| *known.borrow_mut() = js_sys::WeakMap::new());
}

fn remember_all() {
    for (node, element) in crate::document::nodes() {
        if let Some(dom) = element.dom()
            && let Some(object) = dom.dyn_ref::<js_sys::Object>()
        {
            KNOWN.with(|known| known.borrow().set(object, &(node.key() as f64).into()));
        }
    }
}

fn known(dom: &web_sys::Node) -> Option<NodeId> {
    let object = dom.dyn_ref::<js_sys::Object>()?;
    let key = KNOWN.with(|known| known.borrow().get(object)).as_f64()?;
    let node = NodeId::from_key(key as u64);
    crate::document::element(node)
        .and_then(|element| element.dom())
        .is_some_and(|held| &held == dom)
        .then_some(node)
}

pub fn node_of(dom: &web_sys::Node) -> Option<NodeId> {
    known(dom).or_else(|| {
        remember_all();
        known(dom)
    })
}

pub fn node_at_or_above(dom: Option<web_sys::Node>) -> Option<NodeId> {
    let mut next = dom;
    while let Some(dom) = next {
        if let Some(node) = node_of(&dom) {
            return Some(node);
        }
        next = dom.parent_node();
    }
    None
}

pub fn nodes_above(dom: Option<web_sys::Node>) -> Vec<NodeId> {
    let mut path = Vec::new();
    let mut next = dom;
    while let Some(dom) = next {
        if let Some(node) = node_of(&dom)
            && path.last() != Some(&node)
        {
            path.push(node);
        }
        next = dom.parent_node();
    }
    path
}

thread_local! {
    static FRAME: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
}

pub fn request_animation_frame(frame: fn()) {
    let Some(window) = window() else {
        return;
    };
    FRAME.with(|held| {
        let mut held = held.borrow_mut();
        let closure = held.get_or_insert_with(|| Closure::new(move |_: f64| frame()));
        let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
    });
}

pub fn set_timeout(delay: Duration, wake: fn()) {
    let Some(window) = window() else {
        return;
    };
    let closure = Closure::once_into_js(wake);
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.unchecked_ref(),
        i32::try_from(delay.as_millis()).unwrap_or(i32::MAX),
    );
}

pub fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    capture: bool,
    handler: impl FnMut(E) + 'static,
) -> Listener {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    options.set_capture(capture);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
        name,
        closure.as_ref().unchecked_ref(),
        &options,
    );
    Listener {
        target: target.clone(),
        name: name.to_owned(),
        capture,
        closure: closure.into_js_value(),
    }
}

pub struct Listener {
    target: web_sys::EventTarget,
    name: String,
    capture: bool,
    closure: wasm_bindgen::JsValue,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            &self.name,
            self.closure.unchecked_ref(),
            self.capture,
        );
    }
}
