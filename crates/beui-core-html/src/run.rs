use std::cell::Cell;

use beui_html::Node;

use beui_types::Color32;
use reactive::{Get, Set, Signal, create_binding};

use crate::document::{append, create, element, set_attribute, set_style};
use crate::node::NodeId;

pub const PINK: &str = "#ff69b4";

pub const BASE_STYLES: &str = "\
.beui-root { display: grid; grid-template-columns: minmax(0, 1fr); width: 100%; min-height: 100%; }
.beui-root, .beui-root * { box-sizing: border-box; margin: 0; }
.beui-break::after { content: \"\\A\"; white-space: pre; }
.beui-layers > * + * { pointer-events: none; }
.beui-stand-in { outline: 2px dashed #ff69b4; outline-offset: -2px; }
.beui-overlay { position: fixed; inset: auto; margin: 0; padding: 0; border: 0; background: transparent; color: inherit; pointer-events: auto; max-width: none; max-height: none; overflow: visible; }
.beui-overlay::backdrop { background: var(--beui-scrim, transparent); }
.beui-beak { position: absolute; z-index: -1; width: var(--beui-beak); height: var(--beui-beak); top: calc(var(--beui-beak) / -2); left: calc(50% - var(--beui-beak) / 2); transform: rotate(45deg); }
.beui-beak[hidden] { display: none; }
@container anchored(fallback: flip-block) { .beui-beak { top: auto; bottom: calc(var(--beui-beak) / -2); } }
";

pub const STYLES_ID: &str = "beui-styles";

pub fn stylesheet() -> String {
    format!("{BASE_STYLES}{}", crate::fonts::font_faces("."))
}

thread_local! {
    static ROOT: Cell<Option<NodeId>> = const { Cell::new(None) };
    static BACKGROUND: Signal<Option<Color32>> = crate::global_signal(None);
}

pub fn set_background(color: Color32) {
    BACKGROUND.with(|background| background.set(Some(color)));
    crate::page::set_background(color);
}

pub fn root() -> Option<NodeId> {
    ROOT.with(Cell::get)
        .filter(|root| crate::document::contains(*root))
}

fn root_element(build: impl FnOnce() -> NodeId) -> Node {
    let child = build();
    let root = create("Root", "div");
    set_attribute(root, "class", Some("beui-root".to_owned()));
    append(root, child);
    let background = BACKGROUND.with(|background| *background);
    create_binding(move || {
        let color = background.get().filter(|color| color.alpha() > 0);
        set_style(root, "background-color", color.map(crate::css::color));
    });
    ROOT.with(|held| held.set(Some(root)));
    element(root).expect("the root was just built")
}

pub fn render_to_string(build: impl FnOnce() -> NodeId) -> String {
    beui_html::render_fragment(|| root_element(build))
}

#[cfg(target_arch = "wasm32")]
pub use web::{hydrate, run};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::any::Any;
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;

    use super::root_element;
    use crate::node::NodeId;

    thread_local! {
        static RUNNING: RefCell<Vec<Box<dyn Any>>> = RefCell::default();
    }

    fn container(element_id: &str) -> Result<(web_sys::Document, web_sys::Element), String> {
        let document = crate::dom::document().ok_or("no browser document is available")?;
        let container = document
            .get_element_by_id(element_id)
            .ok_or_else(|| format!("no element has the id {element_id}"))?;
        Ok((document, container))
    }

    fn add_styles(document: &web_sys::Document) {
        if document.get_element_by_id(super::STYLES_ID).is_some() {
            return;
        }
        let (Ok(style), Some(head)) = (document.create_element("style"), document.head()) else {
            return;
        };
        style.set_id(super::STYLES_ID);
        style.set_text_content(Some(&super::stylesheet()));
        let _ = head.append_child(&style);
    }

    fn watch_changes(container: &web_sys::Node) -> Option<web_sys::MutationObserver> {
        use wasm_bindgen::closure::Closure;

        let changed = Closure::<dyn FnMut()>::new(|| {
            if crate::measure::watched() {
                crate::frame::request_frame();
            }
        });
        let observer = web_sys::MutationObserver::new(changed.as_ref().unchecked_ref()).ok()?;
        changed.forget();
        let options = web_sys::MutationObserverInit::new();
        options.set_subtree(true);
        options.set_child_list(true);
        options.set_attributes(true);
        options.set_character_data(true);
        observer.observe_with_options(container, &options).ok()?;
        Some(observer)
    }

    fn start(
        document: &web_sys::Document,
        container: &web_sys::Node,
        mounted: beui_html::dom::Mounted,
    ) {
        let mut held: Vec<Box<dyn Any>> = vec![Box::new(mounted)];
        held.push(Box::new(watch_changes(container)));
        held.push(Box::new(crate::focus::watch_document(document)));
        if let Some(window) = crate::dom::window() {
            held.push(Box::new(crate::page::watch(&window)));
        }
        RUNNING.with(|running| running.borrow_mut().extend(held));
        crate::frame::start();
    }

    pub fn run(element_id: &str, build: impl FnOnce() -> NodeId) -> Result<(), String> {
        let (document, container) = container(element_id)?;
        add_styles(&document);
        let container: web_sys::Node = container.unchecked_into();
        let mounted = beui_html::dom::mount(&container, || root_element(build));
        start(&document, &container, mounted);
        Ok(())
    }

    pub fn hydrate(element_id: &str, build: impl FnOnce() -> NodeId) -> Result<(), String> {
        let (document, container) = container(element_id)?;
        add_styles(&document);
        let container: web_sys::Node = container.unchecked_into();
        let mounted = beui_html::dom::hydrate(&container, || root_element(build));
        start(&document, &container, mounted);
        Ok(())
    }
}
