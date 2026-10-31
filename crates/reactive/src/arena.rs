use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::scope::{Cleanup, current_owner};
use crate::signal::Tracked;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Key {
    index: u32,
    generation: u32,
}

struct Slot {
    generation: u32,
    value: Option<Rc<dyn Any>>,
    tracked: Option<Rc<dyn Tracked>>,
}

struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

thread_local! {
    static ARENA: RefCell<Arena> = const {
        RefCell::new(Arena {
            slots: Vec::new(),
            free: Vec::new(),
        })
    };
}

pub(crate) fn insert(value: Rc<dyn Any>) -> Key {
    insert_slot(value, None)
}

pub(crate) fn insert_tracked<V: Tracked + 'static>(value: Rc<V>) -> Key {
    insert_slot(value.clone(), Some(value))
}

fn insert_slot(value: Rc<dyn Any>, tracked: Option<Rc<dyn Tracked>>) -> Key {
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        match arena.free.pop() {
            Some(index) => {
                let slot = &mut arena.slots[index as usize];
                slot.value = Some(value);
                slot.tracked = tracked;
                Key {
                    index,
                    generation: slot.generation,
                }
            }
            None => {
                let index = u32::try_from(arena.slots.len()).expect("too many reactive values");
                arena.slots.push(Slot {
                    generation: 0,
                    value: Some(value),
                    tracked,
                });
                Key {
                    index,
                    generation: 0,
                }
            }
        }
    })
}

pub(crate) fn insert_owned(value: Rc<dyn Any>) -> Key {
    let owner = current_owner();
    let key = insert(value);
    owner.add(Cleanup::Free(key));
    key
}

pub(crate) fn insert_owned_tracked<V: Tracked + 'static>(value: Rc<V>) -> Key {
    insert_owned_slot(value.clone(), Some(value))
}

fn insert_owned_slot(value: Rc<dyn Any>, tracked: Option<Rc<dyn Tracked>>) -> Key {
    let owner = current_owner();
    let key = insert_slot(value, tracked);
    owner.add(Cleanup::Free(key));
    key
}

pub(crate) fn get<T: 'static>(key: Key) -> Option<Rc<T>> {
    get_any(key)?.downcast().ok()
}

fn get_any(key: Key) -> Option<Rc<dyn Any>> {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        let slot = arena.slots.get(key.index as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        slot.value.clone()
    })
}

pub(crate) fn remove(key: Key) {
    let removed = ARENA.try_with(|arena| {
        let mut arena = arena.borrow_mut();
        let slot = arena.slots.get_mut(key.index as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        let value = slot.value.take();
        let tracked = slot.tracked.take();
        slot.generation = slot.generation.wrapping_add(1);
        arena.free.push(key.index);
        Some((value, tracked))
    });
    if let Ok(Some((_, Some(tracked)))) = &removed {
        tracked.source().freed.set(true);
    }
    drop(removed);
}

#[cfg(test)]
pub(crate) fn live() -> usize {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        arena.slots.len() - arena.free.len()
    })
}
