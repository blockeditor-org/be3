use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use crate::runtime::batch;
use crate::scope::Scope;
use crate::signal::{ReadSignal, WriteSignal, create_signal};

struct Entry<V> {
    read: ReadSignal<V>,
    write: WriteSignal<V>,
}

enum Target<V> {
    Unchanged,
    Changed(WriteSignal<V>),
    Missing,
}

struct Inner<K, V> {
    keys: ReadSignal<Vec<K>>,
    write_keys: WriteSignal<Vec<K>>,
    entries: RefCell<HashMap<K, Entry<V>>>,
}

pub struct KeyedStore<K, V> {
    inner: Rc<Inner<K, V>>,
}

impl<K, V> Clone for KeyedStore<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K, V> PartialEq for KeyedStore<K, V> {
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
        let (keys, write_keys) = create_signal(Vec::new());
        Self {
            inner: Rc::new(Inner {
                keys,
                write_keys,
                entries: RefCell::new(HashMap::new()),
            }),
        }
    }

    pub fn keys(&self) -> ReadSignal<Vec<K>> {
        self.inner.keys.clone()
    }

    pub fn try_get(&self, key: &K) -> Option<ReadSignal<V>> {
        self.inner
            .entries
            .borrow()
            .get(key)
            .map(|entry| entry.read.clone())
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
                Some(entry) if entry.read.with_untracked(|current| current == value) => {
                    Target::Unchanged
                }
                Some(entry) => Target::Changed(entry.write.clone()),
            }
        };
        match target {
            Target::Unchanged => {}
            Target::Changed(write) => write.set_unconditionally(value.clone()),
            Target::Missing => {
                let (read, write) = create_signal(value.clone());
                self.inner
                    .entries
                    .borrow_mut()
                    .insert(key.clone(), Entry { read, write });
            }
        }
    }

    fn set_keys(&self, keys: Vec<K>) {
        if self.inner.keys.with_untracked(|current| *current == keys) {
            return;
        }
        let retained: HashSet<&K> = keys.iter().collect();
        self.inner
            .entries
            .borrow_mut()
            .retain(|key, _| retained.contains(key));
        drop(retained);
        self.inner.write_keys.set_unconditionally(keys);
    }
}

pub struct KeyedItems<K, V> {
    build: Box<dyn Fn(K) -> V>,
    entries: RefCell<HashMap<K, (V, Scope)>>,
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
            .get(key)
            .map(|(value, _)| value.clone())
    }

    pub fn entry(&self, key: K) -> V {
        if let Some(value) = self.get(&key) {
            return value;
        }
        let scope = Scope::detached();
        let built = scope.context().run(|| (self.build)(key.clone()));
        self.entries
            .borrow_mut()
            .insert(key, (built.clone(), scope));
        built
    }

    pub fn retain(&self, keys: &[K]) -> Vec<V> {
        let kept: HashSet<&K> = keys.iter().collect();
        let dropped: Vec<K> = self
            .entries
            .borrow()
            .keys()
            .filter(|key| !kept.contains(key))
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
        for key in keys {
            let entry = match entries.remove(&key) {
                Some(entry) => entry,
                None => {
                    let scope = Scope::detached();
                    let built = scope.context().run(|| (self.build)(key.clone()));
                    (built, scope)
                }
            };
            mapped.push(entry.0.clone());
            assert!(
                next.insert(key, entry).is_none(),
                "a keyed list was given the same key twice"
            );
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
    gone: HashMap<K, (V, Scope)>,
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
