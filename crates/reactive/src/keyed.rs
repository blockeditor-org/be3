use foldhash::{HashMap, HashSet};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use crate::scope::Scope;
use crate::signal::{Get, OwnedSignal, ReadSignal, Set};
use crate::stored::{OwnedStored, Stored, store};

mod views;

use views::{Change, Notify};

struct Inner<K: 'static, V: 'static> {
    keys: OwnedSignal<Vec<K>>,
    entries: RefCell<HashMap<K, OwnedSignal<V>>>,
    views: RefCell<Vec<(u64, Rc<dyn Notify<K, V>>)>>,
    next_view: Cell<u64>,
}

impl<K: 'static, V: 'static> Inner<K, V> {
    fn watched(&self) -> bool {
        !self.views.borrow().is_empty()
    }

    fn notify(&self, change: &Change<'_, K, V>) {
        let views: Vec<_> = self
            .views
            .borrow()
            .iter()
            .map(|(_, view)| view.clone())
            .collect();
        for view in views {
            view.changed(change);
        }
    }
}

pub struct KeyedStore<K: 'static, V: 'static> {
    inner: Stored<Inner<K, V>>,
}

impl<K: 'static, V: 'static> Clone for KeyedStore<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static, V: 'static> Copy for KeyedStore<K, V> {}

impl<K: 'static, V: 'static> PartialEq for KeyedStore<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<K: 'static, V: 'static> Eq for KeyedStore<K, V> {}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> Default for KeyedStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

fn empty<K: 'static, V: 'static>() -> Inner<K, V> {
    Inner {
        keys: OwnedSignal::new(Vec::new()),
        entries: RefCell::new(HashMap::default()),
        views: RefCell::new(Vec::new()),
        next_view: Cell::new(0),
    }
}

pub struct OwnedKeyedStore<K: 'static, V: 'static> {
    store: KeyedStore<K, V>,
    _inner: OwnedStored<Inner<K, V>>,
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> Default
    for OwnedKeyedStore<K, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> OwnedKeyedStore<K, V> {
    pub fn new() -> Self {
        let inner = OwnedStored::new(empty());
        Self {
            store: KeyedStore {
                inner: inner.handle(),
            },
            _inner: inner,
        }
    }
}

impl<K: 'static, V: 'static> OwnedKeyedStore<K, V> {
    pub fn handle(&self) -> KeyedStore<K, V> {
        self.store
    }
}

impl<K: 'static, V: 'static> std::ops::Deref for OwnedKeyedStore<K, V> {
    type Target = KeyedStore<K, V>;

    fn deref(&self) -> &KeyedStore<K, V> {
        &self.store
    }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> KeyedStore<K, V> {
    pub fn new() -> Self {
        Self {
            inner: store(empty()),
        }
    }

    pub fn keys(&self) -> ReadSignal<Vec<K>> {
        self.inner.with(|inner| inner.keys.read_only())
    }

    pub fn try_get(&self, key: &K) -> Option<ReadSignal<V>> {
        self.inner.with(|inner| {
            inner
                .entries
                .borrow()
                .get(key)
                .map(|entry| entry.read_only())
        })
    }

    pub fn get(&self, key: &K) -> ReadSignal<V> {
        self.try_get(key)
            .expect("keyed store has no item under this key")
    }

    pub fn contains(&self, key: &K) -> bool {
        self.inner
            .with(|inner| inner.entries.borrow().contains_key(key))
    }

    pub fn tracked_keys(&self) -> usize {
        self.inner.with(|inner| inner.entries.borrow().len())
    }

    pub fn reconcile<'a>(&self, items: impl IntoIterator<Item = (K, &'a V)>)
    where
        V: 'a,
    {
        let mut keys = Vec::new();
        let mut inserted = 0;
        for (key, value) in items {
            inserted += usize::from(self.write(&key, Cow::Borrowed(value)));
            keys.push(key);
        }
        self.set_keys(keys, inserted);
    }

    pub fn reconcile_owned(&self, items: impl IntoIterator<Item = (K, V)>) {
        let mut keys = Vec::new();
        let mut inserted = 0;
        for (key, value) in items {
            inserted += usize::from(self.write(&key, Cow::Owned(value)));
            keys.push(key);
        }
        self.set_keys(keys, inserted);
    }

    pub fn set(&self, key: K, value: V) {
        if self.write(&key, Cow::Owned(value)) {
            self.inner.value().keys.update(|keys| keys.push(key));
        }
    }

    pub fn update<R>(&self, key: &K, f: impl FnOnce(&mut V) -> R) -> Option<R> {
        let mut value = self.try_get(key)?.get_untracked();
        let result = f(&mut value);
        self.write(key, Cow::Owned(value));
        Some(result)
    }

    pub fn remove(&self, key: &K) {
        let inner = self.inner.value();
        let Some(entry) = inner.entries.borrow_mut().remove(key) else {
            return;
        };
        inner.keys.update(|keys| {
            if let Some(at) = keys.iter().position(|held| held == key) {
                keys.remove(at);
            }
        });
        if inner.watched() {
            entry.with_untracked(|old| inner.notify(&Change::Removed(key, old)));
        }
    }

    fn write(&self, key: &K, value: Cow<'_, V>) -> bool {
        let inner = self.inner.value();
        let entry = inner.entries.borrow().get(key).map(|entry| entry.handle());
        match entry {
            Some(entry) => {
                if !entry.with_untracked(|current| *current == *value) {
                    let old =
                        entry.update(|current| std::mem::replace(current, value.into_owned()));
                    if inner.watched() {
                        entry.with_untracked(|new| inner.notify(&Change::Updated(key, &old, new)));
                    }
                }
                false
            }
            None => {
                let entry = OwnedSignal::new(value.into_owned());
                let handle = entry.handle();
                inner.entries.borrow_mut().insert(key.clone(), entry);
                if inner.watched() {
                    handle.with_untracked(|new| inner.notify(&Change::Inserted(key, new)));
                }
                true
            }
        }
    }

    fn set_keys(&self, keys: Vec<K>, inserted: usize) {
        let inner = self.inner.value();
        if inner.keys.with_untracked(|current| *current == keys) {
            return;
        }
        let watched = inner.watched();
        let (removed, reordered) = {
            let retained: HashSet<&K> = keys.iter().collect();
            let mut entries = inner.entries.borrow_mut();
            inner.keys.with_untracked(|old| {
                let removed: Vec<(K, OwnedSignal<V>)> = old
                    .iter()
                    .filter(|key| !retained.contains(key))
                    .filter_map(|key| entries.remove_entry(key))
                    .collect();
                let reordered = watched && {
                    let mut kept = old.iter().filter(|key| retained.contains(key));
                    let survivors = keys.len() - inserted;
                    !keys[..survivors].iter().eq(kept.by_ref()) || kept.next().is_some()
                };
                (removed, reordered)
            })
        };
        if watched {
            for (key, entry) in &removed {
                entry.with_untracked(|old| inner.notify(&Change::Removed(key, old)));
            }
        }
        drop(removed);
        inner.keys.set_unconditionally(keys);
        if reordered {
            inner
                .keys
                .with_untracked(|keys| inner.notify(&Change::Reordered(keys)));
        }
    }
}

pub fn numbered<K: Clone + Hash + Eq>(keys: Vec<K>) -> Vec<(K, usize)> {
    let mut seen: HashMap<K, usize> = HashMap::default();
    keys.into_iter()
        .map(|key| {
            let occurrence = seen.entry(key.clone()).or_insert(0);
            let nth = *occurrence;
            *occurrence += 1;
            (key, nth)
        })
        .collect()
}

pub struct KeyedItems<K, V> {
    build: Box<dyn Fn(K) -> V>,
    entries: RefCell<HashMap<(K, usize), (V, Scope)>>,
    mapped: RefCell<(Vec<K>, Vec<V>)>,
}

impl<K: Clone + Hash + Eq + 'static, V: Clone + 'static> KeyedItems<K, V> {
    pub fn new(build: impl Fn(K) -> V + 'static) -> Self {
        Self {
            build: Box::new(build),
            entries: RefCell::new(HashMap::default()),
            mapped: RefCell::new((Vec::new(), Vec::new())),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.entries
            .borrow()
            .get(&(key.clone(), 0))
            .map(|(value, _)| value.clone())
    }

    pub fn entry(&self, key: K) -> V {
        if let Some(value) = self.get(&key) {
            return value;
        }
        let built = self.build_scoped(key.clone());
        let value = built.0.clone();
        self.entries.borrow_mut().insert((key, 0), built);
        self.forget_mapped();
        value
    }

    pub fn entry_owned(&self, key: K) -> V {
        if let Some(value) = self.get(&key) {
            return value;
        }
        let built = self.build_scoped(key.clone());
        let value = built.0.clone();
        let scope = built.1.context();
        self.entries.borrow_mut().insert((key, 0), built);
        self.forget_mapped();
        scope.flush_owned();
        value
    }

    fn build_scoped(&self, key: K) -> (V, Scope) {
        let scope = Scope::detached();
        let built = scope.context().run(|| (self.build)(key));
        (built, scope)
    }

    fn forget_mapped(&self) {
        *self.mapped.borrow_mut() = (Vec::new(), Vec::new());
    }

    pub fn retain(&self, keys: &[K]) -> Vec<V> {
        self.forget_mapped();
        let kept: HashSet<&K> = keys.iter().collect();
        let dropped: Vec<(K, usize)> = self
            .entries
            .borrow()
            .keys()
            .filter(|(key, _)| !kept.contains(key))
            .cloned()
            .collect();
        drop(kept);
        let mut removed = Vec::with_capacity(dropped.len());
        let mut scopes = Vec::with_capacity(dropped.len());
        {
            let mut entries = self.entries.borrow_mut();
            for key in dropped {
                if let Some((value, scope)) = entries.remove(&key) {
                    removed.push(value);
                    scopes.push(scope);
                }
            }
        }
        drop(scopes);
        removed
    }

    pub fn map(&self, keys: Vec<K>) -> Mapping<K, V> {
        if let Some(mapping) = self.append(&keys) {
            return mapping;
        }
        let mut entries = self.entries.borrow_mut();
        let mut next = HashMap::with_capacity_and_hasher(keys.len(), Default::default());
        let mut mapped = Vec::with_capacity(keys.len());
        for numbered in numbered(keys.clone()) {
            let entry = match entries.remove(&numbered) {
                Some(entry) => entry,
                None => self.build_scoped(numbered.0.clone()),
            };
            mapped.push(entry.0.clone());
            next.insert(numbered, entry);
        }
        let gone = std::mem::replace(&mut *entries, next);
        *self.mapped.borrow_mut() = (keys, mapped.clone());
        Mapping {
            items: mapped,
            gone,
        }
    }

    fn append(&self, keys: &[K]) -> Option<Mapping<K, V>> {
        let mut held = self.mapped.borrow_mut();
        let (before, values) = &mut *held;
        let added = keys.strip_prefix(before.as_slice())?;
        let mut entries = self.entries.borrow_mut();
        let mut fresh: HashSet<&K> = HashSet::default();
        if !added
            .iter()
            .all(|key| fresh.insert(key) && !entries.contains_key(&(key.clone(), 0)))
        {
            return None;
        }
        for key in added {
            let entry = self.build_scoped(key.clone());
            values.push(entry.0.clone());
            entries.insert((key.clone(), 0), entry);
        }
        before.extend_from_slice(added);
        Some(Mapping {
            items: values.clone(),
            gone: HashMap::default(),
        })
    }
}

pub struct Mapping<K, V> {
    items: Vec<V>,
    gone: HashMap<(K, usize), (V, Scope)>,
}

impl<K, V: Clone> Mapping<K, V> {
    pub fn items(&self) -> &[V] {
        &self.items
    }

    pub fn commit(self, write: impl FnOnce(Vec<V>, Vec<V>)) {
        let Mapping { items, gone } = self;
        let removed = gone.values().map(|(item, _)| item.clone()).collect();
        write(items, removed);
        drop(gone);
    }
}
