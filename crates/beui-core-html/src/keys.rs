use std::rc::Rc;

pub use beui_types::input::KeyPhase;
use beui_types::input::{Key, KeyPress};

use crate::node::{NodeId, NodeRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyInput {
    Press(KeyPress),
    Tap(Key),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyTarget {
    pub node: Option<NodeId>,
    pub typing: bool,
    pub app: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub input: KeyInput,
    pub phase: KeyPhase,
    pub target: KeyTarget,
}

impl KeyEvent {
    pub fn key(&self) -> Key {
        match self.input {
            KeyInput::Press(press) => press.key,
            KeyInput::Tap(key) => key,
        }
    }

    pub fn press(&self) -> Option<KeyPress> {
        match self.input {
            KeyInput::Press(press) => Some(press),
            KeyInput::Tap(_) => None,
        }
    }

    pub fn tap(&self) -> Option<Key> {
        match self.input {
            KeyInput::Tap(key) => Some(key),
            KeyInput::Press(_) => None,
        }
    }

    pub fn held(&self) -> bool {
        self.phase == KeyPhase::Held
    }
}

#[derive(Clone)]
pub enum KeyAt {
    Host,
    Window,
    Root,
    Node(NodeRef),
}

pub type KeyHandler = dyn Fn(KeyEvent) -> bool;

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
