use crate::core::selection::RowKey;
use crate::core::selection::RowWatcher;
use crate::core::selection::SelectableRows;
use crate::core::selection::register_rows;
use crate::core::selection::unregister_rows;
use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::OwnedSignal;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::reactive::Scope;
use crate::reactive::Set;
use crate::reactive::create_effect;
use crate::reactive::on_cleanup;
use crate::reactive::owner_scope;
use crate::reactive::untrack;
use beui_core::base::list::Direction;
use beui_core::current::with_document;
use beui_core::node::NodeId;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::rc::Weak;

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

pub(crate) fn virtual_list<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    direction: Prop<Direction>,
    overscan: Prop<f32>,
    text: Callback<K, String>,
    item: RenderFn<ReadSignal<K>>,
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
    let rows = Rc::new(Rows {
        list: Cell::new(None),
        keys: RefCell::new(Vec::new()),
        indices: RefCell::new(None),
        previous: RefCell::new(None),
        pool: Rc::clone(&pool),
        text,
        watchers: RefCell::new(Vec::new()),
    });
    let owner = owner_scope();
    let building = Rc::clone(&pool);
    let announcing = Rc::clone(&rows);
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
            let shown_key = slot.key.with_untracked(Clone::clone);
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
            announcing.announce(node, shown_key);
            node
        })
    });
    rows.list.set(Some(list.id()));
    register_rows(list.id(), Rc::clone(&rows) as Rc<dyn SelectableRows>);
    let listed = list.id();
    on_cleanup(move || unregister_rows(listed));
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
        let changed = rows.set_keys(keys.clone());
        with_document(|document| document.set_virtual_list_keys(list, keys));
        if let Some(previous) = changed {
            untrack(|| rows.keys_changed(previous));
        }
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

struct Rows<K: 'static> {
    list: Cell<Option<NodeId>>,
    keys: RefCell<Vec<K>>,
    indices: RefCell<Option<HashMap<K, usize>>>,
    previous: RefCell<Option<Vec<K>>>,
    pool: Rc<RefCell<Pool<K>>>,
    text: Callback<K, String>,
    watchers: RefCell<Vec<Weak<dyn RowWatcher>>>,
}

impl<K: Clone + Hash + Eq + 'static> Rows<K> {
    fn watchers(&self) -> Vec<Rc<dyn RowWatcher>> {
        let mut watchers = self.watchers.borrow_mut();
        watchers.retain(|watcher| watcher.strong_count() > 0);
        watchers.iter().filter_map(Weak::upgrade).collect()
    }

    fn set_keys(&self, keys: Vec<K>) -> Option<Vec<K>> {
        if *self.keys.borrow() == keys {
            return None;
        }
        let previous = self.keys.replace(keys);
        *self.indices.borrow_mut() = None;
        Some(previous)
    }

    fn keys_changed(&self, previous: Vec<K>) {
        let (watchers, Some(list)) = (self.watchers(), self.list.get()) else {
            return;
        };
        if watchers.is_empty() {
            return;
        }
        *self.previous.borrow_mut() = Some(previous);
        for watcher in watchers {
            watcher.keys_changed(list);
        }
        *self.previous.borrow_mut() = None;
    }

    fn announce(&self, node: NodeId, key: K) {
        let Some(list) = self.list.get() else {
            return;
        };
        for watcher in self.watchers() {
            watcher.row_shown(list, node, RowKey::new(key.clone()));
        }
    }

    fn position(&self, key: &K) -> Option<usize> {
        let mut indices = self.indices.borrow_mut();
        let indices = indices.get_or_insert_with(|| {
            let mut indices = HashMap::with_capacity(self.keys.borrow().len());
            for (index, key) in self.keys.borrow().iter().enumerate() {
                indices.entry(key.clone()).or_insert(index);
            }
            indices
        });
        indices.get(key).copied()
    }
}

impl<K: Clone + Hash + Eq + 'static> SelectableRows for Rows<K> {
    fn count(&self) -> usize {
        self.keys.borrow().len()
    }

    fn key_at(&self, index: usize) -> Option<RowKey> {
        self.keys.borrow().get(index).cloned().map(RowKey::new)
    }

    fn index_of(&self, key: &RowKey) -> Option<usize> {
        self.position(key.get::<K>()?)
    }

    fn shown(&self) -> Vec<(RowKey, NodeId)> {
        self.pool
            .borrow()
            .shown
            .iter()
            .filter_map(|held| {
                let held = held.borrow();
                let slot = held.as_ref()?;
                Some((
                    RowKey::new(slot.key.with_untracked(Clone::clone)),
                    slot.node,
                ))
            })
            .collect()
    }

    fn text(&self, key: &RowKey) -> Option<String> {
        let key = key.get::<K>()?.clone();
        (!self.text.is_empty()).then(|| self.text.call(key))
    }

    fn previous_index(&self, key: &RowKey) -> Option<usize> {
        let key = key.get::<K>()?;
        let previous = self.previous.borrow();
        previous.as_ref()?.iter().position(|shown| shown == key)
    }

    fn replacement(&self, key: &RowKey, forward: bool) -> Option<(RowKey, bool)> {
        let at = self.previous_index(key)?;
        let previous = self.previous.borrow();
        let previous = previous.as_ref()?;
        let after = || {
            previous[at + 1..]
                .iter()
                .find(|key| self.position(key).is_some())
                .map(|key| (RowKey::new(key.clone()), true))
        };
        let before = || {
            previous[..at]
                .iter()
                .rev()
                .find(|key| self.position(key).is_some())
                .map(|key| (RowKey::new(key.clone()), false))
        };
        match forward {
            true => after().or_else(before),
            false => before().or_else(after),
        }
    }

    fn watch(&self, watcher: &Rc<dyn RowWatcher>) {
        let mut watchers = self.watchers.borrow_mut();
        let held = Rc::downgrade(watcher);
        if !watchers.iter().any(|other| Weak::ptr_eq(other, &held)) {
            watchers.push(held);
        }
    }
}
