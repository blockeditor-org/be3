use std::cell::Cell;

use reactive::{Stored, store};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl NodeId {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }

    pub(crate) fn key(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }

    pub(crate) fn from_key(key: u64) -> Self {
        Self {
            index: key as u32,
            generation: (key >> 32) as u32,
        }
    }
}

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
        self.0.try_with(|cell| cell.set(Some(node)));
    }

    pub fn get(&self) -> NodeId {
        self.try_get()
            .expect("node_ref read while the node it points at is not built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0
            .try_with(Cell::get)
            .flatten()
            .filter(|node| crate::document::contains(*node))
    }
}
