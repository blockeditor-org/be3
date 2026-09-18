use crate::node::NodeId;

pub(crate) trait ChildItem {
    fn node(&self) -> NodeId;
}

impl ChildItem for NodeId {
    fn node(&self) -> NodeId {
        *self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotId(u32);

enum SlotItems<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> SlotItems<T> {
    fn as_slice(&self) -> &[T] {
        match self {
            SlotItems::One(item) => std::slice::from_ref(item),
            SlotItems::Many(items) => items,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            SlotItems::One(item) => std::slice::from_mut(item),
            SlotItems::Many(items) => items,
        }
    }
}

struct Slot<T> {
    id: SlotId,
    items: SlotItems<T>,
}

pub(crate) struct ChildList<T> {
    next: u32,
    slots: Vec<Slot<T>>,
}

impl<T> Default for ChildList<T> {
    fn default() -> Self {
        Self {
            next: 0,
            slots: Vec::new(),
        }
    }
}

impl<T> ChildList<T> {
    fn take_id(&mut self) -> SlotId {
        let id = SlotId(self.next);
        self.next += 1;
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.slots
            .iter()
            .map(|slot| slot.items.as_slice().len())
            .sum()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().flat_map(|slot| slot.items.as_slice())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots
            .iter_mut()
            .flat_map(|slot| slot.items.as_mut_slice())
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.iter().nth(index)
    }

    pub(crate) fn push(&mut self, item: T) {
        let id = self.take_id();
        self.slots.push(Slot {
            id,
            items: SlotItems::One(item),
        });
    }

    pub(crate) fn set_all(&mut self, items: Vec<T>) {
        self.slots.clear();
        for item in items {
            self.push(item);
        }
    }

    pub(crate) fn take_all(&mut self) -> Vec<T> {
        let slots = std::mem::take(&mut self.slots);
        slots
            .into_iter()
            .flat_map(|slot| match slot.items {
                SlotItems::One(item) => vec![item],
                SlotItems::Many(items) => items,
            })
            .collect()
    }

    pub(crate) fn open(&mut self) -> SlotId {
        let id = self.take_id();
        self.slots.push(Slot {
            id,
            items: SlotItems::Many(Vec::new()),
        });
        id
    }

    pub(crate) fn fill(&mut self, slot: SlotId, items: Vec<T>) {
        let Some(found) = self.slots.iter_mut().find(|held| held.id == slot) else {
            return;
        };
        found.items = SlotItems::Many(items);
    }
}

impl<T: ChildItem> ChildList<T> {
    pub(crate) fn nodes(&self) -> Vec<NodeId> {
        self.iter().map(ChildItem::node).collect()
    }

    pub(crate) fn find(&self, child: NodeId) -> Option<&T> {
        self.iter().find(|item| item.node() == child)
    }

    pub(crate) fn contains(&self, child: NodeId) -> bool {
        self.iter().any(|item| item.node() == child)
    }

    pub(crate) fn remove(&mut self, child: NodeId) {
        for slot in &mut self.slots {
            if let SlotItems::Many(items) = &mut slot.items {
                items.retain(|item| item.node() != child);
            }
        }
        self.slots.retain(|slot| match &slot.items {
            SlotItems::One(item) => item.node() != child,
            SlotItems::Many(_) => true,
        });
    }

    pub(crate) fn find_mut(&mut self, child: NodeId) -> Option<&mut T> {
        self.iter_mut().find(|item| item.node() == child)
    }
}
