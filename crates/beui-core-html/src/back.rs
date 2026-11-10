//! Back on the HTML core: the browser's back, Android's back and Esc reach `on_back`
//! handlers and dismissable layers as `Dismiss::Back` does natively, as one
//! `BackGesture::Invoked` (the web has no gesture in progress).
//!
//! The target is found as natively: walking the open layers down from the top, skipping
//! inert ones, the latest enabled handler shown in a layer takes it, else the layer itself
//! when it would close on dismiss; a layer that blocks everything stops the walk, and
//! below every layer a handler in the page takes it. Once a frame, while there is a
//! target, the page holds one history entry of its own, marked `beuiBack`, which the
//! back button and Android's back pop; when the target goes the entry is taken back.
//! Browsers skip an entry made without a user activation, so a page may get only one
//! back per activation before back leaves it.
//!
//! Not a `CloseWatcher`: Esc is a close request too, so a watcher would take Esc from the
//! layers' own Escape handling, and give it to handlers in the page, which Escape never
//! reaches natively.

#[cfg(target_arch = "wasm32")]
use std::cell::Cell;
use std::cell::RefCell;

use beui_tree::reactive::Callback;
use beui_types::BackGesture;

use crate::node::NodeRef;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BackHandlerId(u64);

#[cfg_attr(
    not(target_arch = "wasm32"),
    expect(dead_code, reason = "only the browser goes back")
)]
struct Handler {
    id: BackHandlerId,
    at: Option<NodeRef>,
    enabled: bool,
    on_back: Callback<BackGesture>,
}

#[derive(Default)]
struct Handlers {
    handlers: Vec<Handler>,
    next: u64,
}

thread_local! {
    static HANDLERS: RefCell<Handlers> = RefCell::default();
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Whether a handler, a layer or a handler's visibility changed since history was
    /// last brought in step, so a frame with nothing changed does no work.
    static DIRTY: Cell<bool> = const { Cell::new(false) };
}

/// Adds a handler for the component at `at` (the page itself when `None`), enabled from
/// the start or not, so one that starts disabled never touches history.
pub fn add(at: Option<NodeRef>, enabled: bool, on_back: Callback<BackGesture>) -> BackHandlerId {
    let id = HANDLERS.with(|handlers| {
        let mut handlers = handlers.borrow_mut();
        let id = BackHandlerId(handlers.next);
        handlers.next += 1;
        handlers.handlers.push(Handler {
            id,
            at,
            enabled,
            on_back,
        });
        id
    });
    changed();
    id
}

pub fn set_enabled(id: BackHandlerId, enabled: bool) {
    let flipped = HANDLERS.with(|handlers| {
        let mut handlers = handlers.borrow_mut();
        let handler = handlers
            .handlers
            .iter_mut()
            .find(|handler| handler.id == id)?;
        let flipped = handler.enabled != enabled;
        handler.enabled = enabled;
        Some(flipped)
    });
    if flipped == Some(true) {
        changed();
    }
}

pub fn remove(id: BackHandlerId) {
    HANDLERS.with(|handlers| {
        handlers
            .borrow_mut()
            .handlers
            .retain(|handler| handler.id != id)
    });
    changed();
}

/// Handlers and layers change many times a flush; history is brought in step once, on
/// the next frame.
fn changed() {
    #[cfg(target_arch = "wasm32")]
    {
        web::watch();
        web::mark();
    }
}

/// A layer opened, closed or rose, which may change what back reaches.
pub(crate) fn layers_changed() {
    #[cfg(target_arch = "wasm32")]
    {
        web::watch();
        web::mark();
    }
}

/// The page starts: a reload may have left it on its own entry, which is taken over
/// before anything asks for back.
#[cfg(target_arch = "wasm32")]
pub(crate) fn start() {
    web::watch();
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use beui_types::{BackGesture, Dismiss, LayerPolicy};
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::{JsCast, JsValue};

    use super::{DIRTY, HANDLERS};
    use crate::dom::{Listener, call};
    use crate::node::NodeId;
    use crate::overlay::Layer;

    const MARKER: &str = "beuiBack";

    #[derive(Default)]
    struct Held {
        /// The pass that brings history in step each frame, and the popstate listener.
        pass: Option<(Rc<dyn Fn() -> bool>, Listener)>,
    }

    thread_local! {
        static HELD: RefCell<Held> = RefCell::default();
        /// Whether the page's own entry is current.
        static PUSHED: Cell<bool> = const { Cell::new(false) };
        /// `history.length` when the entry was pushed: a popstate that finds it changed
        /// was a navigation forward (a `#` link), not back.
        static LENGTH: Cell<f64> = const { Cell::new(0.0) };
        /// The page took its entry back and the popstate has not come: the next popstate
        /// is that retraction, whatever else it might have been.
        static RETRACTING: Cell<bool> = const { Cell::new(false) };
        /// Watches the handlers' elements for being shown, hidden, moved in or removed,
        /// which `target` depends on and nothing else marks.
        static OBSERVER: RefCell<Option<(JsValue, Closure<dyn FnMut()>)>> = const { RefCell::new(None) };
    }

    pub(super) fn mark() {
        DIRTY.with(|dirty| dirty.set(true));
        crate::frame::request_frame();
    }

    enum Target {
        Handler(beui_tree::reactive::Callback<BackGesture>),
        Layer(Rc<Layer>),
    }

    pub(super) fn watch() {
        let installed = HELD.with(|held| {
            let mut held = held.borrow_mut();
            if held.pass.is_some() {
                return false;
            }
            let Some(window) = crate::dom::window() else {
                return false;
            };
            let pass: Rc<dyn Fn() -> bool> = Rc::new(|| {
                if DIRTY.with(|dirty| dirty.replace(false)) {
                    sync();
                }
                false
            });
            crate::frame::hold_layout(&pass);
            let listener =
                crate::dom::listen(window.as_ref(), "popstate", false, |_: web_sys::Event| {
                    popped()
                });
            held.pass = Some((pass, listener));
            true
        });
        if installed {
            adopt_entry();
        }
    }

    /// Has a handler's element's size or presence change mark history dirty: a handler
    /// hidden by `display`, a tab switch or removal stops counting, a shown one starts.
    fn observe(element: &web_sys::Element) {
        OBSERVER.with(|observer| {
            let mut observer = observer.borrow_mut();
            if observer.is_none() {
                let callback = Closure::<dyn FnMut()>::new(mark);
                let Ok(constructor) =
                    js_sys::Reflect::get(&js_sys::global(), &"ResizeObserver".into())
                else {
                    return;
                };
                let Ok(made) = js_sys::Reflect::construct(
                    constructor.unchecked_ref::<js_sys::Function>(),
                    &js_sys::Array::of1(callback.as_ref()),
                ) else {
                    return;
                };
                *observer = Some((made, callback));
            }
            if let Some((made, _)) = observer.as_ref() {
                call(made, "observe", &[element.clone().into()]);
            }
        });
    }

    /// A reload, or a return without the back-forward cache, may land on the page's own
    /// entry from before: hold it as pushed, so it is taken back or reused, not stranded
    /// under a second one.
    fn adopt_entry() {
        let Some(history) = history() else {
            return;
        };
        if on_own_entry(&history) {
            PUSHED.with(|pushed| pushed.set(true));
            LENGTH.with(|held| held.set(length(&history)));
            mark();
        }
    }

    fn history() -> Option<JsValue> {
        let window = crate::dom::window()?;
        js_sys::Reflect::get(&window, &"history".into()).ok()
    }

    fn length(history: &JsValue) -> f64 {
        js_sys::Reflect::get(history, &"length".into())
            .ok()
            .and_then(|length| length.as_f64())
            .unwrap_or(0.0)
    }

    fn on_own_entry(history: &JsValue) -> bool {
        js_sys::Reflect::get(history, &"state".into())
            .ok()
            .filter(JsValue::is_object)
            .and_then(|state| js_sys::Reflect::get(&state, &MARKER.into()).ok())
            .is_some_and(|marked| marked.is_truthy())
    }

    /// The innermost layer's dialog around `node`, `None` in the page itself.
    fn layer_of(node: NodeId) -> Option<web_sys::Element> {
        let element = crate::dom::element(node)?;
        element.closest(".beui-overlay").ok().flatten()
    }

    fn shown(node: NodeId) -> bool {
        let Some(element) = crate::dom::element(node) else {
            return false;
        };
        element.is_connected()
            && call(element.as_ref(), "checkVisibility", &[]).is_none_or(|shown| shown.is_truthy())
    }

    fn handler_in(dialog: Option<&web_sys::Element>) -> Option<Target> {
        HANDLERS.with(|handlers| {
            handlers
                .borrow()
                .handlers
                .iter()
                .rev()
                .filter(|handler| handler.enabled)
                .find(|handler| match handler.at {
                    Some(at) => at.try_get().is_some_and(|node| {
                        if let Some(element) = crate::dom::element(node) {
                            observe(&element);
                        }
                        shown(node) && layer_of(node).as_ref() == dialog
                    }),
                    None => dialog.is_none(),
                })
                .map(|handler| Target::Handler(handler.on_back))
        })
    }

    fn target() -> Option<Target> {
        for layer in crate::overlay::stacked().into_iter().rev() {
            if layer.policy() == LayerPolicy::Inert {
                continue;
            }
            let dialog = crate::dom::element(layer.dialog());
            if let Some(target) = handler_in(dialog.as_ref()) {
                return Some(target);
            }
            if layer.dismisses() {
                return Some(Target::Layer(layer));
            }
            if layer.blocks_all() {
                return None;
            }
        }
        handler_in(None)
    }

    fn invoke() {
        match target() {
            Some(Target::Handler(on_back)) => on_back.call(BackGesture::Invoked),
            Some(Target::Layer(layer)) => layer.dismiss(Dismiss::Back),
            None => {}
        }
        beui_reactive::flush();
        mark();
    }

    fn sync() {
        let wanted = target().is_some();
        let Some(history) = history() else {
            return;
        };
        let pushed = PUSHED.with(Cell::get);
        if wanted && !pushed {
            if on_own_entry(&history) {
                if RETRACTING.with(Cell::get) {
                    // The entry's retraction is still on its way: it holds again only
                    // if the popstate does not come, and is pushed anew once it has.
                    mark();
                } else {
                    PUSHED.with(|pushed| pushed.set(true));
                    LENGTH.with(|held| held.set(length(&history)));
                }
                return;
            }
            let marker = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&marker, &MARKER.into(), &true.into());
            if call(&history, "pushState", &[marker.into(), "".into()]).is_some() {
                PUSHED.with(|pushed| pushed.set(true));
                LENGTH.with(|held| held.set(length(&history)));
            }
        } else if !wanted && pushed {
            PUSHED.with(|pushed| pushed.set(false));
            if on_own_entry(&history) {
                RETRACTING.with(|retracting| retracting.set(true));
                call(&history, "back", &[]);
            }
        }
    }

    fn popped() {
        let Some(history) = history() else {
            return;
        };
        if RETRACTING.with(|retracting| retracting.replace(false)) {
            // Whatever this popstate was, it ends the retraction, and the entry is gone.
            PUSHED.with(|pushed| pushed.set(false));
            mark();
            return;
        }
        if on_own_entry(&history) {
            // Forward onto the page's own entry: it holds again, and nothing went back.
            PUSHED.with(|pushed| pushed.set(true));
            mark();
            return;
        }
        if !PUSHED.with(|pushed| pushed.replace(false)) {
            return;
        }
        if length(&history) != LENGTH.with(Cell::get) {
            // A navigation forward (a `#` link) left the entry behind; it was no back.
            mark();
            return;
        }
        invoke();
    }
}
