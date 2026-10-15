use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use crate::reactive::{
    OwnedSignal, Prop, ReadSignal, RenderFn, Scope, create_effect, on_cleanup, owner_scope,
    with_document,
};
use beui_core::base::list::Direction;
use beui_core::node::NodeId;
use beui_macros::component;

struct Slot<K: 'static> {
    node: NodeId,
    key: OwnedSignal<K>,
    scope: Scope,
}

struct Pool<K: 'static> {
    free: Vec<Slot<K>>,
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
        live: 0,
        peak: 0,
    }));
    let owner = owner_scope();
    let building = Rc::clone(&pool);
    let list = with_document(|document| {
        document.create_virtual_list(move |key: K| {
            let slot = match reusable(&building, &key) {
                Some(slot) => {
                    if slot.key.read().with_untracked(|shown| *shown != key) {
                        slot.key.write().set(key);
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
                    let read = signal.read();
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
            {
                let mut held = building.borrow_mut();
                held.live += 1;
                held.peak = held.peak.max(held.live);
            }
            let releasing = Rc::clone(&building);
            on_cleanup(move || {
                let mut held = releasing.borrow_mut();
                held.live -= 1;
                if held.free.len() < held.peak {
                    slot.scope.pause();
                    held.free.push(slot);
                }
            });
            node
        })
    });
    on_cleanup(move || pool.borrow_mut().free.clear());
    create_effect(move || {
        with_document(|document| document.set_virtual_list_direction::<K>(list, direction.get()))
    });
    create_effect(move || {
        let keys = keys.get();
        with_document(|document| document.set_virtual_list_keys(list, keys));
    });
    create_effect(move || {
        with_document(|document| document.set_virtual_list_overscan::<K>(list, overscan.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_virtual_list_item_size::<K>(list, item_size.get()))
    });
    list.id()
}

fn reusable<K: Eq>(pool: &Rc<RefCell<Pool<K>>>, key: &K) -> Option<Slot<K>> {
    {
        let mut held = pool.borrow_mut();
        if let Some(at) = held
            .free
            .iter()
            .rposition(|slot| slot.key.read().with_untracked(|shown| shown == key))
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
