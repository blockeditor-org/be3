use crate::node::NodeId;

pub(crate) trait ChildItem {
    fn node(&self) -> NodeId;
}

impl ChildItem for NodeId {
    fn node(&self) -> NodeId {
        *self
    }
}

pub(crate) struct ChildList<T> {
    items: Vec<T>,
}

impl<T> Default for ChildList<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> ChildList<T> {
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub(crate) fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub(crate) fn set_all(&mut self, items: Vec<T>) {
        self.items = items;
    }
}

impl<T: ChildItem> ChildList<T> {
    pub(crate) fn nodes(&self) -> Vec<NodeId> {
        self.items.iter().map(ChildItem::node).collect()
    }

    pub(crate) fn contains(&self, child: NodeId) -> bool {
        self.items.iter().any(|item| item.node() == child)
    }

    pub(crate) fn find(&self, child: NodeId) -> Option<&T> {
        self.items.iter().find(|item| item.node() == child)
    }

    pub(crate) fn find_mut(&mut self, child: NodeId) -> Option<&mut T> {
        self.items.iter_mut().find(|item| item.node() == child)
    }

    pub(crate) fn remove(&mut self, child: NodeId) {
        self.items.retain(|item| item.node() != child);
    }
}
