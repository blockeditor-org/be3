use std::cell::Cell;
use std::rc::Rc;

pub use beui_tree::callback::{Callback, ClickCallback};

use crate::node::NodeId;

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
        crate::current::try_with_document(|document| {
            document.register_node_ref(node, Rc::downgrade(&self.0));
        });
    }

    pub fn get(&self) -> NodeId {
        self.0
            .get()
            .expect("node_ref read while the node it points at is not built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.get()
    }
}
