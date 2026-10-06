use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::reactive::{
    ChildList, ChildValue, Children, ForEach, IntoSlotHost, Prop, Scope, Show, SlotChild, SlotHost,
    SlotId, component, create_effect, create_memo, create_signal, on_cleanup, view,
};

mod a_mock_dom_follows_its_signals_through_show_and_for_each;
mod a_mock_dom_row_that_leaves_disposes_the_effects_it_built;

struct DomElement {
    tag: &'static str,
    text: RefCell<String>,
    children: RefCell<ChildList<DomNode>>,
    scopes: RefCell<Vec<Scope>>,
}

#[derive(Clone)]
struct DomNode(Rc<DomElement>);

impl DomNode {
    fn new(tag: &'static str) -> Self {
        Self(Rc::new(DomElement {
            tag,
            text: RefCell::new(String::new()),
            children: RefCell::new(ChildList::default()),
            scopes: RefCell::new(Vec::new()),
        }))
    }

    fn html(&self) -> String {
        let children: String = self.0.children.borrow().iter().map(DomNode::html).collect();
        let tag = self.0.tag;
        format!("<{tag}>{}{children}</{tag}>", self.0.text.borrow())
    }

    fn release(&self) {
        let scopes = self.0.scopes.take();
        drop(scopes);
        let children: Vec<DomNode> = self.0.children.borrow().iter().cloned().collect();
        for child in children {
            child.release();
        }
    }
}

impl ChildValue for DomNode {
    fn adopt_scope(&mut self, scope: Scope) {
        self.0.scopes.borrow_mut().push(scope);
    }
}

crate::child_type!(DomNode);

impl SlotChild for DomNode {
    type Stored = DomNode;

    fn store(self) -> DomNode {
        self
    }

    fn discard(stored: &DomNode) {
        stored.release();
    }
}

struct DomHost(Weak<DomElement>);

impl DomHost {
    fn with(&self, f: impl FnOnce(&DomElement)) {
        if let Some(element) = self.0.upgrade() {
            f(&element);
        }
    }
}

impl SlotHost<DomNode> for DomHost {
    fn append(&self, stored: DomNode) {
        self.with(|element| element.children.borrow_mut().push(stored));
    }

    fn open(&self) -> SlotId {
        let element = self
            .0
            .upgrade()
            .expect("a slot opens while its element is built");
        element.children.borrow_mut().open()
    }

    fn fill(&self, slot: SlotId, items: Vec<DomNode>) {
        self.with(|element| element.children.borrow_mut().fill(slot, items));
    }

    fn adopt_scope(&self, scope: Scope) {
        self.with(|element| element.scopes.borrow_mut().push(scope));
    }
}

impl IntoSlotHost<DomNode> for &DomNode {
    fn into_slot_host(self) -> Rc<dyn SlotHost<DomNode>> {
        Rc::new(DomHost(Rc::downgrade(&self.0)))
    }
}

#[component]
fn Element(
    tag: &'static str,
    #[prop(default = String::new())] text: Prop<String>,
    texts: Option<Rc<Cell<usize>>>,
    children: Children<DomNode>,
) -> DomNode {
    let element = DomNode::new(tag);
    let shown = Rc::downgrade(&element.0);
    create_effect(move || {
        let text = text.get();
        if let Some(texts) = &texts {
            texts.set(texts.get() + 1);
        }
        if let Some(shown) = shown.upgrade() {
            *shown.text.borrow_mut() = text;
        }
    });
    children.mount(&element);
    element
}
