use std::cell::Cell;

use beui_reactive::{Get, Memo, ReadSignal, Signal, create_memo};

use crate::node::{NodeId, NodeRef};

pub use beui_types::node::ImeCursor;

#[derive(Clone, Copy)]
struct FocusSignals {
    focused: Signal<Option<NodeId>>,
    visible: Signal<bool>,
}

thread_local! {
    static WANTED: Cell<Option<NodeId>> = const { Cell::new(None) };
    static SIGNALS: FocusSignals = FocusSignals {
        focused: crate::global_signal(None),
        visible: crate::global_signal(false),
    };
}

fn signals() -> FocusSignals {
    SIGNALS.with(|signals| *signals)
}

pub fn watch_focused() -> ReadSignal<Option<NodeId>> {
    signals().focused.read_only()
}

pub fn focused_node() -> Option<NodeId> {
    signals().focused.get_untracked()
}

pub fn focus_ring(focused: Memo<bool>) -> Memo<bool> {
    let visible = signals().visible;
    create_memo(move || focused.get() && visible.get())
}

pub fn focus_visible() -> bool {
    signals().visible.get_untracked()
}

pub fn holds_focus(node: NodeRef) -> Memo<bool> {
    let focused = signals().focused;
    create_memo(move || focused.get().is_some() && node.try_get().is_some_and(focus_is_within))
}

pub fn set_focus(node: Option<NodeId>) {
    WANTED.with(|wanted| wanted.set(node));
    apply_wanted();
    crate::frame::request_frame();
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn wanted() -> Option<NodeId> {
    WANTED.with(Cell::get)
}

pub(crate) fn apply_wanted() {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(node) = WANTED.with(Cell::get) else {
            return;
        };
        let Some(element) = crate::dom::html_element(node) else {
            return;
        };
        if !crate::document::contains(node) {
            WANTED.with(|wanted| wanted.set(None));
            return;
        }
        if !element.is_connected() {
            return;
        }
        let holds = || {
            let active = crate::dom::document().and_then(|document| document.active_element());
            let target: &web_sys::Node = element.as_ref();
            active.is_some_and(|active| target.contains(Some(active.as_ref())))
        };
        if !holds() {
            let _ = element.focus();
        }
        if holds() {
            WANTED.with(|wanted| wanted.set(None));
        }
    }
}

pub fn focus_is_within(node: NodeId) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let active = crate::dom::document().and_then(|document| document.active_element());
        crate::dom::dom(node)
            .is_some_and(|dom| active.is_some_and(|active| dom.contains(Some(active.as_ref()))))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = node;
        false
    }
}

pub fn focus_ancestry() -> Vec<NodeId> {
    #[cfg(target_arch = "wasm32")]
    {
        let active = crate::dom::document().and_then(|document| document.active_element());
        crate::dom::nodes_above(active.map(Into::into))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Vec::new()
    }
}

#[cfg(target_arch = "wasm32")]
fn show_focus(visible: bool) {
    use beui_reactive::Set;

    let signal = signals().visible;
    if signal.get_untracked() != visible {
        signal.set(visible);
        beui_reactive::flush();
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn watch_document(document: &web_sys::Document) -> Vec<crate::dom::Listener> {
    use beui_reactive::Set;
    use wasm_bindgen::JsCast;

    let update = |target: Option<web_sys::EventTarget>| {
        let element = target.and_then(|target| target.dyn_into::<web_sys::Element>().ok());
        let visible = element
            .as_ref()
            .is_some_and(|element| element.matches(":focus-visible").unwrap_or(false));
        let node = crate::dom::node_at_or_above(element.map(Into::into));
        let signals = signals();
        if signals.focused.get_untracked() != node {
            signals.focused.set(node);
        }
        if signals.visible.get_untracked() != visible {
            signals.visible.set(visible);
        }
        beui_reactive::flush();
    };
    vec![
        crate::dom::listen(
            document.as_ref(),
            "focusin",
            true,
            move |event: web_sys::FocusEvent| update(event.target()),
        ),
        crate::dom::listen(
            document.as_ref(),
            "keydown",
            true,
            move |event: web_sys::KeyboardEvent| {
                let modifier =
                    beui_web_input::keys::key(&event).is_some_and(|key| key.is_modifier());
                if !modifier && !event.ctrl_key() && !event.alt_key() && !event.meta_key() {
                    show_focus(true);
                }
            },
        ),
        crate::dom::listen(
            document.as_ref(),
            "pointerdown",
            true,
            move |_: web_sys::PointerEvent| show_focus(false),
        ),
        crate::dom::listen(
            document.as_ref(),
            "focusout",
            true,
            move |event: web_sys::FocusEvent| {
                if event.related_target().is_none() {
                    update(None);
                }
            },
        ),
    ]
}
