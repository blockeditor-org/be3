use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::scope::{Cleanup, current_owner};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Key {
    index: u32,
    generation: u32,
}

struct Slot {
    generation: u32,
    value: Option<Rc<dyn Any>>,
}

#[derive(Default)]
struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

thread_local! {
    static ARENA: RefCell<Arena> = RefCell::default();
}

pub(crate) fn insert(value: Rc<dyn Any>) -> Key {
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        match arena.free.pop() {
            Some(index) => {
                let slot = &mut arena.slots[index as usize];
                slot.value = Some(value);
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

pub(crate) fn get<T: 'static>(key: Key) -> Option<Rc<T>> {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        let slot = arena.slots.get(key.index as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        slot.value.clone()?.downcast().ok()
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
        slot.generation = slot.generation.wrapping_add(1);
        arena.free.push(key.index);
        value
    });
    drop(removed);
}

#[cfg(test)]
pub(crate) fn live() -> usize {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        arena.slots.len() - arena.free.len()
    })
}
