use std::cell::Cell;
use std::rc::Rc;

pub use beui_tree::callback::{Callback, ClickCallback};
use reactive::{Stored, store};

use crate::node::NodeId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeRef(Stored<Cell<Option<NodeId>>>);

impl Default for NodeRef {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRef {
    pub fn new() -> Self {
        Self(store(Cell::new(None)))
    }

    pub fn fill(&self, node: NodeId) {
        let Some(cell) = self.0.try_value() else {
            return;
        };
        cell.set(Some(node));
        crate::current::try_with_document(|document| {
            document.register_node_ref(node, Rc::downgrade(&cell));
        });
    }

    pub fn get(&self) -> NodeId {
        self.try_get()
            .expect("node_ref read while the node it points at is not built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.try_with(Cell::get).flatten()
    }
}
