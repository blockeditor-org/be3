use crate::node::NodeId;

pub trait ChildItem {
    fn node(&self) -> NodeId;
}

impl ChildItem for NodeId {
    fn node(&self) -> NodeId {
        *self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotId(u32);

pub trait ChildHost: crate::node::Element {
    type Stored: ChildItem;

    fn children(&mut self) -> &mut ChildList<Self::Stored>;

    fn children_changed(&mut self) {}
}

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

pub struct ChildList<T> {
    next: u32,
    slots: Vec<Slot<T>>,
    revision: u64,
}

impl<T> Default for ChildList<T> {
    fn default() -> Self {
        Self {
            next: 0,
            slots: Vec::new(),
            revision: 0,
        }
    }
}

impl<T> ChildList<T> {
    fn take_id(&mut self) -> SlotId {
        let id = SlotId(self.next);
        self.next += 1;
        self.revision += 1;
        id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.slots
            .iter()
            .map(|slot| slot.items.as_slice().len())
            .sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().flat_map(|slot| slot.items.as_slice())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots
            .iter_mut()
            .flat_map(|slot| slot.items.as_mut_slice())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.iter().nth(index)
    }

    pub fn push(&mut self, item: T) {
        let id = self.take_id();
        self.slots.push(Slot {
            id,
            items: SlotItems::One(item),
        });
    }

    pub fn open(&mut self) -> SlotId {
        let id = self.take_id();
        self.slots.push(Slot {
            id,
            items: SlotItems::Many(Vec::new()),
        });
        id
    }

    pub fn fill(&mut self, slot: SlotId, items: Vec<T>) {
        let Some(found) = self.slots.iter_mut().find(|held| held.id == slot) else {
            return;
        };
        found.items = SlotItems::Many(items);
        self.revision += 1;
    }
}

impl<T: ChildItem> ChildList<T> {
    pub fn nodes(&self) -> Vec<NodeId> {
        self.iter().map(ChildItem::node).collect()
    }

    pub fn find(&self, child: NodeId) -> Option<&T> {
        self.iter().find(|item| item.node() == child)
    }

    pub fn contains(&self, child: NodeId) -> bool {
        self.iter().any(|item| item.node() == child)
    }

    pub fn remove(&mut self, child: NodeId) {
        self.revision += 1;
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

    pub fn find_mut(&mut self, child: NodeId) -> Option<&mut T> {
        self.iter_mut().find(|item| item.node() == child)
    }

    pub fn fill_children(&mut self, slot: SlotId, items: Vec<T>) {
        let Some(index) = self.slots.iter().position(|held| held.id == slot) else {
            return;
        };
        let taken = std::mem::replace(&mut self.slots[index].items, SlotItems::Many(Vec::new()));
        let mut held = match taken {
            SlotItems::One(item) => vec![item],
            SlotItems::Many(items) => items,
        };
        let items = items
            .into_iter()
            .map(
                |item| match held.iter().position(|kept| kept.node() == item.node()) {
                    Some(found) => held.swap_remove(found),
                    None => item,
                },
            )
            .collect();
        self.slots[index].items = SlotItems::Many(items);
        self.revision += 1;
    }
}
