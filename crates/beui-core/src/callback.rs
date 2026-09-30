use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::node::{ClickHandler, Handler, NodeId};

#[derive(Clone, Default)]
pub struct NodeRef(Rc<Cell<Option<NodeId>>>);

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl NodeRef {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(&self, node: NodeId) {
        self.0.set(Some(node));
    }

    pub fn get(&self) -> NodeId {
        self.0
            .get()
            .expect("node_ref read before the node it points at was built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.get()
    }
}

pub struct Callback<V, R = ()>(Rc<RefCell<Option<Handler<V, R>>>>);

impl<V, R> Callback<V, R> {
    pub fn new(handler: impl FnMut(V) -> R + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut(V) -> R + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self, value: V) -> R
    where
        R: Default,
    {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return R::default();
        };
        let result = handler(value);
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
        result
    }
}

impl<V, R> Clone for Callback<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V, R> Default for Callback<V, R> {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Default)]
pub struct ClickCallback(Rc<RefCell<Option<ClickHandler>>>);

impl ClickCallback {
    pub fn new(handler: impl FnMut() + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut() + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self) {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return;
        };
        handler();
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
    }
}
