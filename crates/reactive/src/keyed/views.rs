use foldhash::HashMap;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::ops::{Add, Sub};
use std::rc::{Rc, Weak};

use super::KeyedStore;
use crate::computation::{Computation, no_computation};
use crate::runtime::{Context, leave_component_body};
use crate::scope::on_cleanup;
use crate::signal::{Get, ReadSignal, Set, Signal, create_signal};

pub(crate) enum Change<'a, K, V> {
    Inserted(&'a K, &'a V),
    Updated(&'a K, &'a V, &'a V),
    Removed(&'a K, &'a V),
    Reordered(&'a [K]),
}

pub(crate) trait Notify<K, V> {
    fn changed(&self, change: &Change<'_, K, V>);
}

type Read<'a, K, V> = &'a dyn Fn(&K, &mut dyn FnMut(&V));

trait Derive<K, V>: 'static {
    type Output: Default + PartialEq + 'static;

    fn rebuild(&mut self, keys: &[K], read: Read<'_, K, V>) -> Self::Output;

    fn apply(&mut self, change: &Change<'_, K, V>, output: Signal<Self::Output>);
}

struct View<K: 'static, V: 'static, D: Derive<K, V>> {
    store: KeyedStore<K, V>,
    derive: RefCell<D>,
    output: Signal<D::Output>,
    computation: RefCell<Weak<Computation>>,
}

impl<K: Clone + Eq + Hash + 'static, V: 'static, D: Derive<K, V>> View<K, V, D> {
    fn rebuild(&self) {
        let Some(inner) = self.store.inner.try_value() else {
            return;
        };
        let output = inner.keys.with_untracked(|keys| {
            let entries = inner.entries.borrow();
            self.derive.borrow_mut().rebuild(keys, &|key, read| {
                if let Some(entry) = entries.get(key) {
                    entry.with_untracked(|value| read(value));
                }
            })
        });
        self.output.set(output);
    }
}

impl<K: 'static, V: 'static, D: Derive<K, V>> Notify<K, V> for View<K, V, D> {
    fn changed(&self, change: &Change<'_, K, V>) {
        let Some(computation) = self.computation.borrow().upgrade() else {
            return;
        };
        if computation.is_dirty() || computation.is_disposed() {
            return;
        }
        let _context = Context::enter(Rc::downgrade(&computation), Weak::new());
        let _body = leave_component_body();
        self.derive.borrow_mut().apply(change, self.output);
    }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + PartialEq + 'static> KeyedStore<K, V> {
    pub fn sum_by<T>(&self, f: impl Fn(&V) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + Default + PartialEq + Add<Output = T> + Sub<Output = T> + 'static,
    {
        self.derive(Sum {
            f,
            total: T::default(),
        })
    }

    pub fn max_by<T: Ord + Clone + 'static>(
        &self,
        f: impl Fn(&V) -> T + 'static,
    ) -> ReadSignal<Option<T>> {
        self.derive(Extreme {
            f,
            counts: BTreeMap::new(),
            max: true,
        })
    }

    pub fn min_by<T: Ord + Clone + 'static>(
        &self,
        f: impl Fn(&V) -> T + 'static,
    ) -> ReadSignal<Option<T>> {
        self.derive(Extreme {
            f,
            counts: BTreeMap::new(),
            max: false,
        })
    }

    pub fn sorted_by_key<S: Ord + Clone + 'static>(
        &self,
        f: impl Fn(&V) -> S + 'static,
    ) -> ReadSignal<Vec<K>> {
        self.derive(Sorted {
            f,
            items: HashMap::default(),
            order: Vec::new(),
            next: 0,
        })
    }

    pub fn filter(&self, f: impl Fn(&V) -> bool + 'static) -> ReadSignal<Vec<K>> {
        self.derive(Filter {
            f,
            items: HashMap::default(),
            kept: Vec::new(),
            next: 0,
        })
    }

    fn derive<D: Derive<K, V>>(&self, derive: D) -> ReadSignal<D::Output> {
        let output = create_signal(D::Output::default());
        let view = Rc::new(View {
            store: *self,
            derive: RefCell::new(derive),
            output,
            computation: RefCell::new(no_computation()),
        });
        let rebuild = view.clone();
        let computation = Computation::kept_binding(move || {
            rebuild.rebuild();
            false
        });
        view.computation.replace(Rc::downgrade(&computation));
        let id = self.inner.with(|inner| {
            let id = inner.next_view.get();
            inner.next_view.set(id + 1);
            inner.views.borrow_mut().push((id, view));
            id
        });
        let store = *self;
        on_cleanup(move || {
            store.inner.try_with(|inner| {
                inner.views.borrow_mut().retain(|(held, _)| *held != id);
            });
        });
        computation.refresh();
        output.read_only()
    }

    #[cfg(test)]
    pub(crate) fn views(&self) -> usize {
        self.inner.with(|inner| inner.views.borrow().len())
    }
}

struct Sum<F, T> {
    f: F,
    total: T,
}

impl<K, V, F, T> Derive<K, V> for Sum<F, T>
where
    F: Fn(&V) -> T + 'static,
    T: Clone + Default + PartialEq + Add<Output = T> + Sub<Output = T> + 'static,
{
    type Output = T;

    fn rebuild(&mut self, keys: &[K], read: Read<'_, K, V>) -> T {
        let f = &self.f;
        let mut total = T::default();
        for key in keys {
            read(key, &mut |value| {
                total = std::mem::take(&mut total) + f(value)
            });
        }
        self.total = total.clone();
        total
    }

    fn apply(&mut self, change: &Change<'_, K, V>, output: Signal<T>) {
        let f = &self.f;
        let total = std::mem::take(&mut self.total);
        self.total = match change {
            Change::Inserted(_, value) => total + f(value),
            Change::Updated(_, old, new) => {
                let (old, new) = (f(old), f(new));
                if old == new { total } else { total - old + new }
            }
            Change::Removed(_, old) => total - f(old),
            Change::Reordered(_) => total,
        };
        output.set(self.total.clone());
    }
}

struct Extreme<F, T> {
    f: F,
    counts: BTreeMap<T, usize>,
    max: bool,
}

impl<F, T: Ord + Clone> Extreme<F, T> {
    fn add(&mut self, value: T) {
        *self.counts.entry(value).or_insert(0) += 1;
    }

    fn take(&mut self, value: &T) {
        if let Some(count) = self.counts.get_mut(value) {
            *count -= 1;
            if *count == 0 {
                self.counts.remove(value);
            }
        }
    }

    fn current(&self) -> Option<T> {
        let found = match self.max {
            true => self.counts.last_key_value(),
            false => self.counts.first_key_value(),
        };
        found.map(|(value, _)| value.clone())
    }
}

impl<K, V, F, T> Derive<K, V> for Extreme<F, T>
where
    F: Fn(&V) -> T + 'static,
    T: Ord + Clone + 'static,
{
    type Output = Option<T>;

    fn rebuild(&mut self, keys: &[K], read: Read<'_, K, V>) -> Option<T> {
        self.counts.clear();
        for key in keys {
            read(key, &mut |value| {
                let value = (self.f)(value);
                self.add(value);
            });
        }
        self.current()
    }

    fn apply(&mut self, change: &Change<'_, K, V>, output: Signal<Option<T>>) {
        match change {
            Change::Inserted(_, value) => {
                let value = (self.f)(value);
                self.add(value);
            }
            Change::Updated(_, old, new) => {
                let (old, new) = ((self.f)(old), (self.f)(new));
                if old == new {
                    return;
                }
                self.take(&old);
                self.add(new);
            }
            Change::Removed(_, old) => {
                let old = (self.f)(old);
                self.take(&old);
            }
            Change::Reordered(_) => return,
        }
        output.set(self.current());
    }
}

fn move_within<T>(items: &mut [T], from: usize, to: usize) {
    if from < to {
        items[from..=to].rotate_left(1);
    } else {
        items[to..=from].rotate_right(1);
    }
}

struct Sorted<K, F, S> {
    f: F,
    items: HashMap<K, (S, u64)>,
    order: Vec<(S, u64)>,
    next: u64,
}

impl<K: Clone + Eq + Hash, F, S: Ord + Clone> Sorted<K, F, S> {
    fn arrange(&mut self, keys: &[K]) -> Vec<K> {
        let mut arranged: Vec<(S, u64, K)> = keys
            .iter()
            .filter_map(|key| {
                let (sort, seq) = self.items.get(key)?;
                Some((sort.clone(), *seq, key.clone()))
            })
            .collect();
        arranged.sort_unstable_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        self.order.clear();
        arranged
            .into_iter()
            .map(|(sort, seq, key)| {
                self.order.push((sort, seq));
                key
            })
            .collect()
    }

    fn position(&self, sort: &S, seq: u64) -> Result<usize, usize> {
        self.order
            .binary_search_by(|(held, at)| (held, *at).cmp(&(sort, seq)))
    }
}

impl<K, V, F, S> Derive<K, V> for Sorted<K, F, S>
where
    K: Clone + Eq + Hash + 'static,
    F: Fn(&V) -> S + 'static,
    S: Ord + Clone + 'static,
{
    type Output = Vec<K>;

    fn rebuild(&mut self, keys: &[K], read: Read<'_, K, V>) -> Vec<K> {
        self.items.clear();
        self.next = 0;
        for key in keys {
            read(key, &mut |value| {
                let sort = (self.f)(value);
                self.items.insert(key.clone(), (sort, self.next));
                self.next += 1;
            });
        }
        self.arrange(keys)
    }

    fn apply(&mut self, change: &Change<'_, K, V>, output: Signal<Vec<K>>) {
        match change {
            Change::Inserted(key, value) => {
                let sort = (self.f)(value);
                let seq = self.next;
                self.next += 1;
                let at = self.position(&sort, seq).unwrap_or_else(|at| at);
                self.order.insert(at, (sort.clone(), seq));
                self.items.insert((*key).clone(), (sort, seq));
                output.update(|keys| keys.insert(at, (*key).clone()));
            }
            Change::Updated(key, _, new) => {
                let sort = (self.f)(new);
                let Some((held, seq)) = self.items.get_mut(*key) else {
                    return;
                };
                if *held == sort {
                    return;
                }
                let previous = std::mem::replace(held, sort.clone());
                let seq = *seq;
                let Ok(from) = self.position(&previous, seq) else {
                    return;
                };
                self.order.remove(from);
                let to = self.position(&sort, seq).unwrap_or_else(|at| at);
                self.order.insert(to, (sort, seq));
                if from != to {
                    output.update(|keys| move_within(keys, from, to));
                }
            }
            Change::Removed(key, _) => {
                let Some((sort, seq)) = self.items.remove(*key) else {
                    return;
                };
                if let Ok(at) = self.position(&sort, seq) {
                    self.order.remove(at);
                    output.update(|keys| {
                        keys.remove(at);
                    });
                }
            }
            Change::Reordered(keys) => {
                for (seq, key) in keys.iter().enumerate() {
                    if let Some(held) = self.items.get_mut(key) {
                        held.1 = seq as u64;
                    }
                }
                self.next = keys.len() as u64;
                let arranged = self.arrange(keys);
                output.set(arranged);
            }
        }
    }
}

struct Filter<K, F> {
    f: F,
    items: HashMap<K, (bool, u64)>,
    kept: Vec<u64>,
    next: u64,
}

impl<K: Clone + Eq + Hash, F> Filter<K, F> {
    fn arrange(&mut self, keys: &[K]) -> Vec<K> {
        self.kept.clear();
        keys.iter()
            .filter(|key| {
                let Some((passes, seq)) = self.items.get(*key) else {
                    return false;
                };
                if *passes {
                    self.kept.push(*seq);
                }
                *passes
            })
            .cloned()
            .collect()
    }
}

impl<K, V, F> Derive<K, V> for Filter<K, F>
where
    K: Clone + Eq + Hash + 'static,
    F: Fn(&V) -> bool + 'static,
{
    type Output = Vec<K>;

    fn rebuild(&mut self, keys: &[K], read: Read<'_, K, V>) -> Vec<K> {
        self.items.clear();
        self.next = 0;
        for key in keys {
            read(key, &mut |value| {
                let passes = (self.f)(value);
                self.items.insert(key.clone(), (passes, self.next));
                self.next += 1;
            });
        }
        self.arrange(keys)
    }

    fn apply(&mut self, change: &Change<'_, K, V>, output: Signal<Vec<K>>) {
        match change {
            Change::Inserted(key, value) => {
                let passes = (self.f)(value);
                let seq = self.next;
                self.next += 1;
                self.items.insert((*key).clone(), (passes, seq));
                if passes {
                    self.kept.push(seq);
                    output.update(|keys| keys.push((*key).clone()));
                }
            }
            Change::Updated(key, _, new) => {
                let passes = (self.f)(new);
                let Some((held, seq)) = self.items.get_mut(*key) else {
                    return;
                };
                if *held == passes {
                    return;
                }
                *held = passes;
                let seq = *seq;
                match (self.kept.binary_search(&seq), passes) {
                    (Err(at), true) => {
                        self.kept.insert(at, seq);
                        output.update(|keys| keys.insert(at, (*key).clone()));
                    }
                    (Ok(at), false) => {
                        self.kept.remove(at);
                        output.update(|keys| {
                            keys.remove(at);
                        });
                    }
                    _ => {}
                }
            }
            Change::Removed(key, _) => {
                let Some((passes, seq)) = self.items.remove(*key) else {
                    return;
                };
                if passes && let Ok(at) = self.kept.binary_search(&seq) {
                    self.kept.remove(at);
                    output.update(|keys| {
                        keys.remove(at);
                    });
                }
            }
            Change::Reordered(keys) => {
                for (seq, key) in keys.iter().enumerate() {
                    if let Some(held) = self.items.get_mut(key) {
                        held.1 = seq as u64;
                    }
                }
                self.next = keys.len() as u64;
                let arranged = self.arrange(keys);
                output.set(arranged);
            }
        }
    }
}
