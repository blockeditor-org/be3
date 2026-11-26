//! A `Replica` on the HTML core: a copy of another node's elements, made again whenever they
//! change, while the node itself is drawn at opacity 0 (`[data-beui-lent]` in the base
//! styles) in its place, keeping its size, its input and its focus. The copy is inert and
//! hidden from assistive technology, and holds no test ids, so a test still finds the node.

use std::cell::RefCell;

use crate::node::NodeId;

pub struct Replica {
    element: NodeId,
    held: RefCell<Option<Held>>,
}

struct Held {
    of: NodeId,
    #[cfg(target_arch = "wasm32")]
    watch: web::Watch,
}

pub fn replica(element: NodeId) -> Replica {
    crate::document::set_attribute(element, "class", Some("beui-replica".to_owned()));
    crate::document::set_attribute(element, "inert", Some(String::new()));
    crate::document::set_attribute(element, "aria-hidden", Some("true".to_owned()));
    Replica {
        element,
        held: RefCell::default(),
    }
}

impl Replica {
    /// Copies `of` here, `None` nothing.
    pub fn hold(&self, of: Option<NodeId>) {
        if self.held.borrow().as_ref().map(|held| held.of) == of {
            return;
        }
        self.release();
        let Some(of) = of else {
            return;
        };
        // Rendered as a page, the copy is the node's markup as it is now; in a browser, its
        // elements, copied again as they change.
        #[cfg(not(target_arch = "wasm32"))]
        crate::document::set_markup(
            self.element,
            crate::document::element(of).map(|element| element.to_html_with(&copied)),
        );
        crate::document::set_attribute(of, "data-beui-lent", Some(String::new()));
        *self.held.borrow_mut() = Some(Held {
            of,
            #[cfg(target_arch = "wasm32")]
            watch: web::watch(self.element, of),
        });
    }

    /// Lets go of the node copied, which is drawn in its place again.
    pub fn release(&self) {
        let Some(held) = self.held.take() else {
            return;
        };
        crate::document::set_attribute(held.of, "data-beui-lent", None);
        #[cfg(not(target_arch = "wasm32"))]
        crate::document::set_markup(self.element, None);
        #[cfg(target_arch = "wasm32")]
        {
            drop(held.watch);
            if let Some(into) = crate::dom::dom(self.element) {
                into.set_text_content(None);
            }
        }
    }
}

/// How a copy's elements differ from the node's, given each element's attributes and
/// whether it is the copy's root: no ids or test ids, which name only the node copied, and a
/// root not marked lent, placed by the replica's grid rather than where the node sits in its
/// parent.
fn copied(root: bool, attributes: &mut Vec<(String, String)>) {
    attributes.retain(|(name, _)| {
        !matches!(name.as_str(), "id" | "data-testid") && !(root && name == "data-beui-lent")
    });
    if !root {
        return;
    }
    const PLACED: &str = "grid-area: 1 / 1 / 2 / 2; margin: 0;";
    match attributes.iter_mut().find(|(name, _)| name == "style") {
        Some((_, style)) => {
            let style_end = style.trim_end();
            *style = match style_end.is_empty() || style_end.ends_with(';') {
                true => format!("{style_end} {PLACED}"),
                false => format!("{style_end}; {PLACED}"),
            }
            .trim_start()
            .to_owned();
        }
        None => attributes.push(("style".to_owned(), PLACED.to_owned())),
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use crate::node::NodeId;

    /// The copy kept of a node, once the replica's element is in the page.
    pub(super) struct Watch {
        live: Rc<RefCell<Option<Live>>>,
    }

    struct Live {
        observer: web_sys::MutationObserver,
        _changed: Closure<dyn FnMut()>,
    }

    impl Drop for Live {
        fn drop(&mut self) {
            self.observer.disconnect();
        }
    }

    /// Copies `of` into `replica`, and again whenever anything within `of` changes. A replica
    /// built in this update has no element in the page until it is placed there, so then it
    /// copies once the update is done.
    pub(super) fn watch(replica: NodeId, of: NodeId) -> Watch {
        let live = Rc::new(RefCell::new(None));
        if !start(replica, of, &live) {
            let waiting = Rc::downgrade(&live);
            let later = Closure::once_into_js(move || {
                if let Some(live) = waiting.upgrade() {
                    start(replica, of, &live);
                }
            });
            if let Some(window) = crate::dom::window() {
                window.queue_microtask(later.unchecked_ref());
            }
        }
        Watch { live }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            self.live.borrow_mut().take();
        }
    }

    fn start(replica: NodeId, of: NodeId, live: &Rc<RefCell<Option<Live>>>) -> bool {
        let (Some(into), Some(source)) = (crate::dom::dom(replica), crate::dom::dom(of)) else {
            return false;
        };
        copy(&into, &source);
        let changed = {
            let (into, source) = (into.clone(), source.clone());
            Closure::<dyn FnMut()>::new(move || copy(&into, &source))
        };
        let Ok(observer) = web_sys::MutationObserver::new(changed.as_ref().unchecked_ref()) else {
            return true;
        };
        let options = web_sys::MutationObserverInit::new();
        options.set_subtree(true);
        options.set_child_list(true);
        options.set_attributes(true);
        options.set_character_data(true);
        if observer.observe_with_options(&source, &options).is_ok() {
            *live.borrow_mut() = Some(Live {
                observer,
                _changed: changed,
            });
        }
        true
    }

    fn copy(into: &web_sys::Node, source: &web_sys::Node) {
        let Ok(copied) = source.clone_node_with_deep(true) else {
            return;
        };
        if let Some(root) = copied.dyn_ref::<web_sys::Element>() {
            rewrite(root, true);
            // Sized as the node is where it sits: a canvas's scale is inherited there, not
            // here.
            let scale = source
                .dyn_ref::<web_sys::Element>()
                .and_then(|source| crate::dom::window()?.get_computed_style(source).ok()?)
                .and_then(|style| style.get_property_value("--beui-scale").ok())
                .filter(|scale| !scale.trim().is_empty());
            if let (Some(scale), Some(root)) = (scale, copied.dyn_ref::<web_sys::HtmlElement>()) {
                let _ = root.style().set_property("--beui-scale", scale.trim());
            }
        }
        into.set_text_content(None);
        let _ = into.append_child(&copied);
    }

    /// Gives a copied element and those within it the attributes `copied` says.
    fn rewrite(element: &web_sys::Element, root: bool) {
        let mut attributes: Vec<(String, String)> = element
            .get_attribute_names()
            .iter()
            .filter_map(|name| name.as_string())
            .filter_map(|name| Some((name.clone(), element.get_attribute(&name)?)))
            .collect();
        let before: Vec<String> = attributes.iter().map(|(name, _)| name.clone()).collect();
        super::copied(root, &mut attributes);
        for name in before {
            if !attributes.iter().any(|(kept, _)| *kept == name) {
                let _ = element.remove_attribute(&name);
            }
        }
        for (name, value) in &attributes {
            if element.get_attribute(name).as_ref() != Some(value) {
                let _ = element.set_attribute(name, value);
            }
        }
        let mut child = element.first_element_child();
        while let Some(held) = child {
            rewrite(&held, false);
            child = held.next_element_sibling();
        }
    }
}
