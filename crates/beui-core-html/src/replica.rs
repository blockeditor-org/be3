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
            crate::document::element(of).map(|element| element.to_html()),
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
            let _ = root.remove_attribute("data-beui-lent");
            // Placed by the replica's grid, not by where the node sits in its parent.
            if let Some(style) = copied.dyn_ref::<web_sys::HtmlElement>().map(|root| root.style())
            {
                let _ = style.set_property("grid-area", "1 / 1 / 2 / 2");
                let _ = style.set_property("margin", "0");
            }
            unnamed(root);
        }
        into.set_text_content(None);
        let _ = into.append_child(&copied);
    }

    /// Takes the ids and test ids off a copy, so they name only the node copied.
    fn unnamed(element: &web_sys::Element) {
        let _ = element.remove_attribute("id");
        let _ = element.remove_attribute("data-testid");
        let mut child = element.first_element_child();
        while let Some(held) = child {
            unnamed(&held);
            child = held.next_element_sibling();
        }
    }
}
