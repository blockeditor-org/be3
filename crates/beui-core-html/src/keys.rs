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

#[cfg(target_arch = "wasm32")]
thread_local! {
    static MENU_KEYED: std::cell::RefCell<Option<web_sys::KeyboardEvent>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watch_menu_key(document: &web_sys::Document) -> [crate::dom::Listener; 3] {
    let keyed = crate::dom::listen(
        document.as_ref(),
        "keydown",
        true,
        |event: web_sys::KeyboardEvent| {
            let menu = match beui_web_input::keys::key(&event) {
                Some(beui_types::input::Key::ContextMenu) => true,
                Some(beui_types::input::Key::F10) => event.shift_key(),
                _ => false,
            };
            MENU_KEYED.with(|keyed| *keyed.borrow_mut() = menu.then_some(event));
        },
    );
    let pointed = crate::dom::listen(
        document.as_ref(),
        "pointerdown",
        true,
        |_: web_sys::PointerEvent| MENU_KEYED.with(|keyed| *keyed.borrow_mut() = None),
    );
    let answered = crate::dom::listen(
        document.as_ref(),
        "contextmenu",
        false,
        |event: web_sys::MouseEvent| {
            let taken = MENU_KEYED.with(|keyed| {
                keyed
                    .borrow()
                    .as_ref()
                    .is_some_and(|keyed| keyed.default_prevented())
            });
            if taken {
                event.prevent_default();
            }
        },
    );
    [keyed, pointed, answered]
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn from_menu_key(event: &web_sys::MouseEvent) -> bool {
    event.button() == -1 || MENU_KEYED.with(|keyed| keyed.borrow().is_some())
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
            KeyAt::Control(node) => {
                let held = node.try_get().and_then(crate::dom::element)?;
                let control = held
                    .parent_element()
                    .and_then(|parent| parent.closest("[tabindex]").ok().flatten());
                Some(control.unwrap_or(held).into())
            }
        }
    }

    fn admits(at: &KeyAt, event: &web_sys::KeyboardEvent) -> bool {
        let KeyAt::Control(node) = at else {
            return true;
        };
        let Some(held) = node.try_get().and_then(crate::dom::dom) else {
            return false;
        };
        let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
        else {
            return false;
        };
        held.contains(Some(&target))
            || event
                .current_target()
                .and_then(|current| current.dyn_into::<web_sys::Node>().ok())
                .is_some_and(|current| current == target)
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
                    let at = at.clone();
                    crate::dom::listen(&wanted, name, capture, move |event| {
                        if admits(&at, &event) {
                            deliver(event, phase, &*handler)
                        }
                    })
                });
                (wanted, listeners)
            });
            false
        }));
    }
}
