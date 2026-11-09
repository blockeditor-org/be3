//! `on_back` on the HTML core: the browser's back. While any handler is enabled the
//! page holds one history entry of its own, so the browser's back (a `popstate`)
//! pops that entry and reaches the latest enabled handler as `BackGesture::Invoked`
//! instead of leaving the page. Once none is enabled the entry is taken back.

use std::cell::{Cell, RefCell};

use beui_tree::reactive::Callback;
use beui_types::BackGesture;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BackHandlerId(u64);

struct Handler {
    id: BackHandlerId,
    enabled: bool,
    #[cfg_attr(not(target_arch = "wasm32"), expect(dead_code, reason = "only the browser goes back"))]
    on_back: Callback<BackGesture>,
}

#[derive(Default)]
struct Handlers {
    handlers: Vec<Handler>,
    next: u64,
}

thread_local! {
    static HANDLERS: RefCell<Handlers> = RefCell::default();
    /// Whether the page's own history entry is on top.
    static PUSHED: Cell<bool> = const { Cell::new(false) };
    /// A `popstate` the page caused itself by taking its entry back.
    static RETRACTING: Cell<bool> = const { Cell::new(false) };
}

pub fn add(on_back: Callback<BackGesture>) -> BackHandlerId {
    let id = HANDLERS.with(|handlers| {
        let mut handlers = handlers.borrow_mut();
        let id = BackHandlerId(handlers.next);
        handlers.next += 1;
        handlers.handlers.push(Handler {
            id,
            enabled: true,
            on_back,
        });
        id
    });
    sync();
    id
}

pub fn set_enabled(id: BackHandlerId, enabled: bool) {
    HANDLERS.with(|handlers| {
        if let Some(handler) = handlers
            .borrow_mut()
            .handlers
            .iter_mut()
            .find(|handler| handler.id == id)
        {
            handler.enabled = enabled;
        }
    });
    sync();
}

pub fn remove(id: BackHandlerId) {
    HANDLERS.with(|handlers| {
        handlers
            .borrow_mut()
            .handlers
            .retain(|handler| handler.id != id)
    });
    sync();
}

fn wanted() -> bool {
    HANDLERS.with(|handlers| handlers.borrow().handlers.iter().any(|handler| handler.enabled))
}

/// The browser went back: the latest enabled handler hears it, if the entry it
/// popped was the page's own.
#[cfg_attr(not(target_arch = "wasm32"), expect(dead_code, reason = "only the browser goes back"))]
fn popped() {
    if RETRACTING.with(|retracting| retracting.replace(false)) {
        return;
    }
    if !PUSHED.with(|pushed| pushed.replace(false)) {
        return;
    }
    let on_back = HANDLERS.with(|handlers| {
        handlers
            .borrow()
            .handlers
            .iter()
            .rev()
            .find(|handler| handler.enabled)
            .map(|handler| handler.on_back)
    });
    if let Some(on_back) = on_back {
        on_back.call(BackGesture::Invoked);
    }
    beui_reactive::flush();
    sync();
}

fn sync() {
    let wanted = wanted();
    let pushed = PUSHED.with(Cell::get);
    if wanted != pushed {
        #[cfg(target_arch = "wasm32")]
        web::sync(wanted);
        #[cfg(not(target_arch = "wasm32"))]
        PUSHED.with(|held| held.set(wanted));
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use super::{PUSHED, RETRACTING};
    use crate::dom::Listener;

    thread_local! {
        static LISTENER: RefCell<Option<Listener>> = const { RefCell::new(None) };
    }

    pub(super) fn sync(wanted: bool) {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let Ok(history) = js_sys::Reflect::get(&window, &"history".into()) else {
            return;
        };
        LISTENER.with(|listener| {
            listener.borrow_mut().get_or_insert_with(|| {
                crate::dom::listen(window.as_ref(), "popstate", false, |_: web_sys::Event| {
                    super::popped()
                })
            });
        });
        if wanted {
            let marker = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&marker, &"beuiBack".into(), &true.into());
            crate::dom::call(&history, "pushState", &[marker.into(), "".into()]);
        } else {
            RETRACTING.with(|retracting| retracting.set(true));
            crate::dom::call(&history, "back", &[]);
        }
        PUSHED.with(|pushed| pushed.set(wanted));
    }
}
