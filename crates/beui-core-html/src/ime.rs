use std::rc::Rc;

use beui_types::Rect;
use beui_types::input::{ImeText, Input};

use crate::node::NodeId;

pub type ImeSink = Rc<dyn Fn(Vec<Input>)>;

pub fn begin(node: NodeId, sink: ImeSink) {
    #[cfg(target_arch = "wasm32")]
    web::begin(node, sink);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (node, sink);
}

pub fn end(node: NodeId) {
    #[cfg(target_arch = "wasm32")]
    web::end(node);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = node;
}

pub fn sync(text: Option<&ImeText>) {
    #[cfg(target_arch = "wasm32")]
    web::sync(text);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

pub fn keyboard(node: NodeId, keyboard: beui_types::SoftKeyboard) {
    #[cfg(target_arch = "wasm32")]
    web::keyboard(node, keyboard);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (node, keyboard);
}

pub fn show_keyboard(shown: bool) {
    #[cfg(target_arch = "wasm32")]
    web::show_keyboard(shown);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = shown;
}

pub fn place(rect: Option<Rect>) {
    #[cfg(target_arch = "wasm32")]
    web::place(rect);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = rect;
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use beui_types::Rect;
    use beui_types::input::{ImeEvent, ImeText, Input};
    use beui_web_input::ime_mirror::{ImeInput, ImeMirror};

    use super::ImeSink;
    use crate::css::px;
    use crate::dom::Listener;
    use crate::node::NodeId;

    struct Agent {
        element: web_sys::HtmlTextAreaElement,
        _listeners: Vec<Listener>,
    }

    thread_local! {
        static AGENT: RefCell<Option<Agent>> = const { RefCell::new(None) };
        static MIRROR: RefCell<ImeMirror> = RefCell::new(ImeMirror::default());
        static SESSION: RefCell<Option<(NodeId, ImeSink)>> = const { RefCell::new(None) };
    }

    fn deliver(change: impl FnOnce(&mut ImeMirror, &mut Vec<Input>)) {
        let mut events = Vec::new();
        MIRROR.with(|mirror| change(&mut mirror.borrow_mut(), &mut events));
        if events.is_empty() {
            return;
        }
        let sink = SESSION.with(|session| session.borrow().as_ref().map(|(_, sink)| sink.clone()));
        if let Some(sink) = sink {
            sink(events);
            reactive::flush();
        }
    }

    fn selection(agent: &web_sys::HtmlTextAreaElement) -> (u32, u32) {
        let start = agent.selection_start().ok().flatten().unwrap_or(0);
        let end = agent.selection_end().ok().flatten().unwrap_or(start);
        (start, end)
    }

    fn agent() -> Option<web_sys::HtmlTextAreaElement> {
        AGENT.with(|held| {
            let mut held = held.borrow_mut();
            if held.is_none() {
                let document = crate::dom::document()?;
                let element = beui_web_input::text_agent(&document)?;
                let target: &web_sys::EventTarget = element.as_ref();
                let listeners = vec![
                    crate::dom::listen(target, "input", false, {
                        let agent = element.clone();
                        move |event: web_sys::InputEvent| {
                            let value = agent.value();
                            let input = ImeInput {
                                value: &value,
                                selection: selection(&agent),
                                composing: event.is_composing(),
                                replacement: event.input_type() == "insertReplacementText",
                            };
                            deliver(|mirror, events| mirror.input(input, events));
                        }
                    }),
                    crate::dom::listen(target, "compositionstart", false, |_: web_sys::Event| {
                        MIRROR.with(|mirror| mirror.borrow_mut().start_composition());
                        deliver(|_, events| events.push(Input::Ime(ImeEvent::Enabled)));
                    }),
                    crate::dom::listen(target, "compositionend", false, |_: web_sys::Event| {
                        deliver(|mirror, events| {
                            mirror.end_composition(events);
                            events.push(Input::Ime(ImeEvent::Disabled));
                        });
                    }),
                    crate::dom::listen(document.as_ref(), "selectionchange", false, {
                        let agent = element.clone();
                        move |_: web_sys::Event| {
                            let focused = crate::dom::document()
                                .and_then(|document| document.active_element())
                                .is_some_and(|active| {
                                    let element: &web_sys::Element = agent.as_ref();
                                    active == *element
                                });
                            if focused {
                                let selection = selection(&agent);
                                deliver(|mirror, events| mirror.select(selection, events));
                            }
                        }
                    }),
                ];
                *held = Some(Agent {
                    element,
                    _listeners: listeners,
                });
            }
            held.as_ref().map(|agent| agent.element.clone())
        })
    }

    pub(super) fn begin(node: NodeId, sink: ImeSink) {
        let (Some(agent), Some(host)) = (agent(), crate::dom::dom(node)) else {
            return;
        };
        SESSION.with(|session| *session.borrow_mut() = Some((node, sink)));
        let agent_node: &web_sys::Node = agent.as_ref();
        if agent_node.parent_node().as_ref() != Some(&host) {
            let _ = host.append_child(&agent);
        }
        let active = crate::dom::document().and_then(|document| document.active_element());
        let element: &web_sys::Element = agent.as_ref();
        if active.as_ref() != Some(element) {
            let _ = agent.focus();
        }
    }

    pub(super) fn keyboard(node: NodeId, keyboard: beui_types::SoftKeyboard) {
        use beui_types::SoftKeyboard;

        let current = SESSION.with(|session| {
            session
                .borrow()
                .as_ref()
                .is_some_and(|(held, _)| *held == node)
        });
        let Some(agent) = agent().filter(|_| current) else {
            return;
        };
        let policy = match keyboard {
            SoftKeyboard::OnFocus => "auto",
            SoftKeyboard::OnPress | SoftKeyboard::Held | SoftKeyboard::Shown { .. } => "manual",
        };
        let _ = agent.set_attribute("virtualkeyboardpolicy", policy);
        match keyboard {
            SoftKeyboard::Shown { .. } => show_keyboard(true),
            SoftKeyboard::Held => show_keyboard(false),
            SoftKeyboard::OnFocus | SoftKeyboard::OnPress => {}
        }
    }

    pub(super) fn show_keyboard(shown: bool) {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let Ok(keyboard) = js_sys::Reflect::get(&window.navigator(), &"virtualKeyboard".into())
        else {
            return;
        };
        if keyboard.is_object() {
            crate::dom::call(&keyboard, if shown { "show" } else { "hide" }, &[]);
        }
    }

    pub(super) fn end(node: NodeId) {
        let ended = SESSION.with(|session| {
            let mut session = session.borrow_mut();
            if session.as_ref().is_some_and(|(held, _)| *held == node) {
                *session = None;
                true
            } else {
                false
            }
        });
        if ended {
            MIRROR.with(|mirror| mirror.borrow_mut().sync(None));
        }
    }

    pub(super) fn sync(text: Option<&ImeText>) {
        let Some(agent) = agent() else {
            return;
        };
        let mirrored = MIRROR.with(|mirror| {
            let mut mirror = mirror.borrow_mut();
            mirror
                .sync(text)
                .then(|| (mirror.value().to_owned(), mirror.selection()))
        });
        if let Some((value, (start, end))) = mirrored {
            agent.set_value(&value);
            let _ = agent.set_selection_range(start, end);
        }
    }

    pub(super) fn place(rect: Option<Rect>) {
        let Some(agent) = agent() else {
            return;
        };
        let rect = rect.unwrap_or(Rect::ZERO);
        let style = agent.style();
        let _ = style.set_property("left", &px(rect.min.x));
        let _ = style.set_property("top", &px(rect.min.y));
        let _ = style.set_property("height", &px(rect.height().max(1.0)));
    }
}
