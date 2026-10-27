use std::rc::Rc;

pub use beui_types::input::KeyPhase;

thread_local! {
    static HELD: reactive::Signal<beui_types::Modifiers> = crate::global_signal(beui_types::Modifiers::default());
}

pub fn held_modifiers() -> reactive::ReadSignal<beui_types::Modifiers> {
    HELD.with(|held| held.read_only())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watch_modifiers(document: &web_sys::Document) -> Vec<crate::dom::Listener> {
    use reactive::{Get, Set};

    let update = |event: &web_sys::KeyboardEvent| {
        let held = beui_web_input::keys::modifiers(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        let signal = HELD.with(|held| *held);
        if signal.get_untracked() != held {
            signal.set(held);
            reactive::flush();
        }
    };
    let mut listeners: Vec<crate::dom::Listener> = ["keydown", "keyup"]
        .into_iter()
        .map(|name| {
            crate::dom::listen(
                document.as_ref(),
                name,
                true,
                move |event: web_sys::KeyboardEvent| update(&event),
            )
        })
        .collect();
    if let Some(window) = crate::dom::window() {
        listeners.push(crate::dom::listen(
            window.as_ref(),
            "blur",
            false,
            |_: web_sys::Event| {
                let signal = HELD.with(|held| *held);
                if signal.get_untracked() != beui_types::Modifiers::default() {
                    signal.set(beui_types::Modifiers::default());
                    reactive::flush();
                }
            },
        ));
    }
    listeners
}

pub use beui_types::keys::{KeyAt, KeyEvent, KeyHandler, KeyInput, KeyTarget};

pub fn listen(at: KeyAt, phase: KeyPhase, handler: Rc<KeyHandler>) {
    if phase == KeyPhase::Held {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    web::listen(at, phase, handler);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (at, handler);
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use beui_types::input::{KeyPhase, KeyPress};
    use wasm_bindgen::JsCast;

    use super::{KeyAt, KeyEvent, KeyHandler, KeyInput, KeyTarget};
    use crate::dom::Listener;

    fn target(at: &KeyAt) -> Option<web_sys::EventTarget> {
        match at {
            KeyAt::Host | KeyAt::Window => crate::dom::window().map(Into::into),
            KeyAt::Root => crate::run::root().and_then(crate::dom::dom).map(Into::into),
            KeyAt::Node(node) => node.try_get().and_then(crate::dom::dom).map(Into::into),
        }
    }

    fn deliver(event: web_sys::KeyboardEvent, phase: KeyPhase, handler: &KeyHandler) {
        if event.is_composing() {
            return;
        }
        let Some(key) = beui_web_input::keys::key(&event) else {
            return;
        };
        let modifiers = beui_web_input::keys::modifiers(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        let pressed = event.type_() == "keydown";
        let node = crate::dom::node_at_or_above(
            event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Node>().ok()),
        );
        let typing = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
            .is_some();
        let handled = handler(KeyEvent {
            input: KeyInput::Press(KeyPress {
                key,
                pressed,
                repeat: pressed && event.repeat(),
                modifiers,
            }),
            phase,
            target: KeyTarget {
                node,
                typing,
                app: false,
            },
        });
        if handled {
            event.prevent_default();
            event.stop_propagation();
        }
        reactive::flush();
    }

    pub(super) fn listen(at: KeyAt, phase: KeyPhase, handler: Rc<KeyHandler>) {
        let attached: RefCell<Option<(web_sys::EventTarget, [Listener; 2])>> = RefCell::default();
        let capture = phase == KeyPhase::Capture;
        crate::frame::on_layout(Rc::new(move || {
            let wanted = target(&at);
            let mut attached = attached.borrow_mut();
            if attached.as_ref().map(|(held, _)| held) == wanted.as_ref() {
                return false;
            }
            *attached = wanted.map(|wanted| {
                let listeners = ["keydown", "keyup"].map(|name| {
                    let handler = Rc::clone(&handler);
                    crate::dom::listen(&wanted, name, capture, move |event| {
                        deliver(event, phase, &*handler)
                    })
                });
                (wanted, listeners)
            });
            false
        }));
    }
}
