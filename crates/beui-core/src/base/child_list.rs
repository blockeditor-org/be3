pub use beui_tree::child_list::{ChildList, SlotId};

use crate::node::NodeId;

pub trait ChildItem {
    fn node(&self) -> NodeId;
}

impl ChildItem for NodeId {
    fn node(&self) -> NodeId {
        *self
    }
}

pub trait ChildHost: crate::node::Element {
    type Stored: ChildItem;

    fn children(&mut self) -> &mut ChildList<Self::Stored>;

    fn children_changed(&mut self) {}
}

pub trait NodeChildren<T> {
    fn nodes(&self) -> Vec<NodeId>;
    fn find(&self, child: NodeId) -> Option<&T>;
    fn contains(&self, child: NodeId) -> bool;
    fn remove(&mut self, child: NodeId);
    fn find_mut(&mut self, child: NodeId) -> Option<&mut T>;
    fn fill_children(&mut self, slot: SlotId, items: Vec<T>);
}

impl<T: ChildItem> NodeChildren<T> for ChildList<T> {
    fn nodes(&self) -> Vec<NodeId> {
        self.iter().map(ChildItem::node).collect()
    }

    fn find(&self, child: NodeId) -> Option<&T> {
        self.iter().find(|item| item.node() == child)
    }

    fn contains(&self, child: NodeId) -> bool {
        self.iter().any(|item| item.node() == child)
    }

    fn remove(&mut self, child: NodeId) {
        self.retain(|item| item.node() != child);
    }

    fn find_mut(&mut self, child: NodeId) -> Option<&mut T> {
        self.iter_mut().find(|item| item.node() == child)
    }

    fn fill_children(&mut self, slot: SlotId, items: Vec<T>) {
        self.fill_keeping(slot, items, |kept, item| kept.node() == item.node());
    }
}
