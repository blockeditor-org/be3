use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use crate::reactive::{
    Get, OwnedSignal, Prop, ReadSignal, RenderFn, Scope, Set, create_effect, on_cleanup,
    owner_scope, with_document,
};
use beui_core::base::list::Direction;
use beui_core::node::NodeId;
use beui_macros::component;

struct Slot<K: 'static> {
    node: NodeId,
    key: OwnedSignal<K>,
    scope: Scope,
}

type Held<K> = Rc<RefCell<Option<Slot<K>>>>;

struct Pool<K: 'static> {
    free: Vec<Slot<K>>,
    shown: Vec<Held<K>>,
    live: usize,
    peak: usize,
}

#[component]
pub fn VirtualList<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = 0.0)] overscan: Prop<f32>,
    #[prop(children)] item: RenderFn<ReadSignal<K>>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let pool = Rc::new(RefCell::new(Pool::<K> {
        free: Vec::new(),
        shown: Vec::new(),
        live: 0,
        peak: 0,
    }));
    let owner = owner_scope();
    let building = Rc::clone(&pool);
    let list = with_document(|document| {
        document.create_virtual_list(move |key: K| {
            let slot = match reusable(&building, &key) {
                Some(slot) => {
                    if slot.key.with_untracked(|shown| *shown != key) {
                        slot.key.set(key);
                    }
                    slot.scope.resume();
                    slot.scope.flush_owned();
                    slot
                }
                None => {
                    let scope = owner
                        .as_ref()
                        .and_then(|owner| owner.child())
                        .unwrap_or_else(Scope::detached)
                        .pausable();
                    let signal = OwnedSignal::new(key);
                    let read = signal.read_only();
                    let node = scope.run(|| item.call(read));
                    scope.flush_owned();
                    Slot {
                        node,
                        key: signal,
                        scope,
                    }
                }
            };
            let node = slot.node;
            let shown: Held<K> = Rc::new(RefCell::new(Some(slot)));
            {
                let mut held = building.borrow_mut();
                held.live += 1;
                held.peak = held.peak.max(held.live);
                held.shown.push(Rc::clone(&shown));
            }
            let releasing = Rc::clone(&building);
            on_cleanup(move || {
                let mut held = releasing.borrow_mut();
                held.shown.retain(|other| !Rc::ptr_eq(other, &shown));
                let Some(slot) = shown.borrow_mut().take() else {
                    return;
                };
                held.live -= 1;
                if held.free.len() < held.peak {
                    slot.scope.pause();
                    held.free.push(slot);
                }
            });
            node
        })
    });
    on_cleanup(move || {
        let (free, shown) = {
            let mut held = pool.borrow_mut();
            (
                std::mem::take(&mut held.free),
                std::mem::take(&mut held.shown),
            )
        };
        drop(free);
        for slot in shown {
            drop(slot.borrow_mut().take());
        }
    });
    create_effect(move || {
        let direction = direction.get();
        with_document(|document| document.set_virtual_list_direction::<K>(list, direction))
    });
    create_effect(move || {
        let keys = keys.get();
        with_document(|document| document.set_virtual_list_keys(list, keys));
    });
    create_effect(move || {
        let overscan = overscan.get();
        with_document(|document| document.set_virtual_list_overscan::<K>(list, overscan))
    });
    create_effect(move || {
        let item_size = item_size.get();
        with_document(|document| document.set_virtual_list_item_size::<K>(list, item_size))
    });
    list.id()
}

fn reusable<K: Eq>(pool: &Rc<RefCell<Pool<K>>>, key: &K) -> Option<Slot<K>> {
    {
        let mut held = pool.borrow_mut();
        if let Some(at) = held
            .free
            .iter()
            .rposition(|slot| slot.key.with_untracked(|shown| shown == key))
        {
            return Some(held.free.swap_remove(at));
        }
    }
    loop {
        let slot = pool.borrow_mut().free.pop()?;
        if !with_document(|document| document.focus_is_within(slot.node)) {
            return Some(slot);
        }
    }
}
