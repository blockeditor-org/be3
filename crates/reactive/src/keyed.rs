use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use crate::runtime::batch;
use crate::scope::Scope;
use crate::signal::{OwnedSignal, ReadSignal, WriteSignal};

enum Target<V> {
    Unchanged,
    Changed(WriteSignal<V>),
    Missing,
}

struct Inner<K: 'static, V: 'static> {
    keys: OwnedSignal<Vec<K>>,
    entries: RefCell<HashMap<K, OwnedSignal<V>>>,
}

pub struct KeyedStore<K: 'static, V: 'static> {
    inner: Rc<Inner<K, V>>,
}

impl<K: 'static, V: 'static> Clone for KeyedStore<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K: 'static, V: 'static> PartialEq for KeyedStore<K, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> Default for KeyedStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> KeyedStore<K, V> {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                keys: OwnedSignal::new(Vec::new()),
                entries: RefCell::new(HashMap::new()),
            }),
        }
    }

    pub fn keys(&self) -> ReadSignal<Vec<K>> {
        self.inner.keys.read()
    }

    pub fn try_get(&self, key: &K) -> Option<ReadSignal<V>> {
        self.inner.entries.borrow().get(key).map(OwnedSignal::read)
    }

    pub fn get(&self, key: &K) -> ReadSignal<V> {
        self.try_get(key)
            .expect("keyed store has no item under this key")
    }

    pub fn contains(&self, key: &K) -> bool {
        self.inner.entries.borrow().contains_key(key)
    }

    pub fn tracked_keys(&self) -> usize {
        self.inner.entries.borrow().len()
    }

    pub fn reconcile<'a>(&self, items: impl IntoIterator<Item = (K, &'a V)>)
    where
        V: 'a,
    {
        batch(|| {
            let mut keys = Vec::new();
            for (key, value) in items {
                self.write(&key, value);
                keys.push(key);
            }
            self.set_keys(keys);
        });
    }

    pub fn reconcile_owned(&self, items: impl IntoIterator<Item = (K, V)>) {
        batch(|| {
            let mut keys = Vec::new();
            for (key, value) in items {
                self.write(&key, &value);
                keys.push(key);
            }
            self.set_keys(keys);
        });
    }

    fn write(&self, key: &K, value: &V) {
        let target = {
            let entries = self.inner.entries.borrow();
            match entries.get(key) {
                None => Target::Missing,
                Some(entry) if entry.read().with_untracked(|current| current == value) => {
                    Target::Unchanged
                }
                Some(entry) => Target::Changed(entry.write()),
            }
        };
        match target {
            Target::Unchanged => {}
            Target::Changed(write) => write.set_unconditionally(value.clone()),
            Target::Missing => {
                self.inner
                    .entries
                    .borrow_mut()
                    .insert(key.clone(), OwnedSignal::new(value.clone()));
            }
        }
    }

    fn set_keys(&self, keys: Vec<K>) {
        if self
            .inner
            .keys
            .read()
            .with_untracked(|current| *current == keys)
        {
            return;
        }
        let retained: HashSet<&K> = keys.iter().collect();
        self.inner
            .entries
            .borrow_mut()
            .retain(|key, _| retained.contains(key));
        drop(retained);
        self.inner.keys.write().set_unconditionally(keys);
    }
}

pub fn numbered<K: Clone + Hash + Eq>(keys: Vec<K>) -> Vec<(K, usize)> {
    let mut seen: HashMap<K, usize> = HashMap::new();
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
}

impl<K: Clone + Hash + Eq + 'static, V: Clone + 'static> KeyedItems<K, V> {
    pub fn new(build: impl Fn(K) -> V + 'static) -> Self {
        Self {
            build: Box::new(build),
            entries: RefCell::new(HashMap::new()),
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
        scope.flush_owned();
        value
    }

    fn build_scoped(&self, key: K) -> (V, Scope) {
        let scope = Scope::detached();
        let built = scope.context().run(|| (self.build)(key));
        (built, scope)
    }

    pub fn retain(&self, keys: &[K]) -> Vec<V> {
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
        let mut entries = self.entries.borrow_mut();
        let mut next = HashMap::with_capacity(keys.len());
        let mut mapped = Vec::with_capacity(keys.len());
        for numbered in numbered(keys) {
            let entry = match entries.remove(&numbered) {
                Some(entry) => entry,
                None => self.build_scoped(numbered.0.clone()),
            };
            mapped.push(entry.0.clone());
            next.insert(numbered, entry);
        }
        let gone = std::mem::replace(&mut *entries, next);
        Mapping {
            items: mapped,
            gone,
        }
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
