use beui_tree::child_list::{ChildList, SlotId};

use beui_types::NodeId;

pub trait ChildItem {
    fn node(&self) -> Option<NodeId>;
}

impl ChildItem for NodeId {
    fn node(&self) -> Option<NodeId> {
        Some(*self)
    }
}

pub trait ChildHost: crate::node::Element {
    type Stored: ChildItem;

    fn children(&mut self) -> &mut ChildList<Self::Stored>;

    fn children_changed(&mut self) {}
}

pub fn remove_hosted<H: ChildHost>(host: &mut H, child: NodeId) {
    let children = ChildHost::children(host);
    if children.contains(child) {
        children.remove(child);
        host.children_changed();
    }
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
        self.iter().filter_map(ChildItem::node).collect()
    }

    fn find(&self, child: NodeId) -> Option<&T> {
        self.find_by(child.key(), item_key)
    }

    fn contains(&self, child: NodeId) -> bool {
        self.find(child).is_some()
    }

    fn remove(&mut self, child: NodeId) {
        self.retain(|item| item.node() != Some(child));
    }

    fn find_mut(&mut self, child: NodeId) -> Option<&mut T> {
        self.find_by_mut(child.key(), item_key)
    }

    fn fill_children(&mut self, slot: SlotId, items: Vec<T>) {
        self.fill_keeping(slot, items, item_key);
    }
}

fn item_key<T: ChildItem>(item: &T) -> Option<u64> {
    item.node().map(NodeId::key)
}
