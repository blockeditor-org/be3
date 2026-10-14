use std::cell::RefCell;
use std::collections::HashMap;

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

type Positions = HashMap<u64, (usize, usize)>;

pub struct ChildList<T> {
    next: u32,
    slots: Vec<Slot<T>>,
    revision: u64,
    index: RefCell<Option<(u64, Positions)>>,
}

impl<T> Default for ChildList<T> {
    fn default() -> Self {
        Self {
            next: 0,
            slots: Vec::new(),
            revision: 0,
            index: RefCell::new(None),
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

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.revision += 1;
        self.slots
            .iter_mut()
            .flat_map(|slot| slot.items.as_mut_slice())
    }

    fn locate(&self, key: u64, key_of: impl Fn(&T) -> Option<u64>) -> Option<(usize, usize)> {
        let mut held = self.index.borrow_mut();
        if held
            .as_ref()
            .is_none_or(|(revision, _)| *revision != self.revision)
        {
            let mut index = HashMap::new();
            for (slot, held) in self.slots.iter().enumerate() {
                for (position, item) in held.items.as_slice().iter().enumerate() {
                    if let Some(key) = key_of(item) {
                        index.entry(key).or_insert((slot, position));
                    }
                }
            }
            *held = Some((self.revision, index));
        }
        held.as_ref()?.1.get(&key).copied()
    }

    pub fn find_by(&self, key: u64, key_of: impl Fn(&T) -> Option<u64>) -> Option<&T> {
        let (slot, position) = self.locate(key, key_of)?;
        self.slots[slot].items.as_slice().get(position)
    }

    pub fn find_by_mut(&mut self, key: u64, key_of: impl Fn(&T) -> Option<u64>) -> Option<&mut T> {
        let (slot, position) = self.locate(key, key_of)?;
        self.slots[slot].items.as_mut_slice().get_mut(position)
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

    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.revision += 1;
        for slot in &mut self.slots {
            if let SlotItems::Many(items) = &mut slot.items {
                items.retain(|item| keep(item));
            }
        }
        self.slots.retain(|slot| match &slot.items {
            SlotItems::One(item) => keep(item),
            SlotItems::Many(_) => true,
        });
    }

    pub fn fill_keeping(&mut self, slot: SlotId, items: Vec<T>, same: impl Fn(&T, &T) -> bool) {
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
                |item| match held.iter().position(|kept| same(kept, &item)) {
                    Some(found) => held.swap_remove(found),
                    None => item,
                },
            )
            .collect();
        self.slots[index].items = SlotItems::Many(items);
        self.revision += 1;
    }
}
