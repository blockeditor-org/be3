//! A cache bounded by the total weight of what it holds, letting go of what was used longest ago
//! first: what devices and the server keep of access logs they checked or read
//! (be-block-client's `ServedLogs`, be-server's `access::checked`), each weighed by its entries.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// Values by key, at most `capacity` in total weight. Using or putting one makes it the most
/// recent; putting one past `capacity` lets go of the least recent until the rest fits. A value
/// heavier than `capacity` alone is not kept. Each call is O(log n).
pub struct Lru<K, V> {
    held: HashMap<K, Held<V>>,
    /// Each key by when it was last used, oldest first.
    order: BTreeMap<u64, K>,
    weight: usize,
    capacity: usize,
    next: u64,
}

struct Held<V> {
    used: u64,
    weight: usize,
    value: V,
}

impl<K: Clone + Eq + Hash, V> Lru<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            held: HashMap::new(),
            order: BTreeMap::new(),
            weight: 0,
            capacity,
            next: 0,
        }
    }

    /// The value held for `key`, now the most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let next = self.tick();
        let held = self.held.get_mut(key)?;
        self.order.remove(&held.used);
        held.used = next;
        self.order.insert(next, key.clone());
        Some(&held.value)
    }

    /// Holds `value` for `key`, weighing `weight`, and lets go of the least recently used until
    /// the total fits.
    pub fn insert(&mut self, key: K, value: V, weight: usize) {
        self.remove(&key);
        if weight > self.capacity {
            return;
        }
        let used = self.tick();
        self.order.insert(used, key.clone());
        self.held.insert(
            key,
            Held {
                used,
                weight,
                value,
            },
        );
        self.weight += weight;
        while self.weight > self.capacity {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(held) = self.held.remove(&oldest) {
                self.weight -= held.weight;
            }
        }
    }

    pub fn remove(&mut self, key: &K) {
        if let Some(held) = self.held.remove(key) {
            self.order.remove(&held.used);
            self.weight -= held.weight;
        }
    }

    pub fn clear(&mut self) {
        self.held.clear();
        self.order.clear();
        self.weight = 0;
    }

    pub fn len(&self) -> usize {
        self.held.len()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// The total weight held.
    pub fn weight(&self) -> usize {
        self.weight
    }

    fn tick(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}
