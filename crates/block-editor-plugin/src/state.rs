use stable_hash::HashMap;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unready {
    Loading,
    Failed(String),
}

#[must_use = "dropping a subscription unsubscribes its callback"]
pub struct Subscription(Option<Box<dyn FnOnce()>>);

impl Subscription {
    pub(crate) fn new(cancel: impl FnOnce() + 'static) -> Self {
        Self(Some(Box::new(cancel)))
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            cancel();
        }
    }
}

type Held<K, F> = HashMap<K, Vec<(u64, Rc<F>)>>;

pub(crate) struct Callbacks<K, F: ?Sized> {
    held: Rc<RefCell<(u64, Held<K, F>)>>,
}

impl<K, F: ?Sized> Default for Callbacks<K, F> {
    fn default() -> Self {
        Self {
            held: Rc::new(RefCell::new((0, HashMap::default()))),
        }
    }
}

impl<K: Clone + Eq + Hash + 'static, F: ?Sized + 'static> Callbacks<K, F> {
    pub(crate) fn add(&self, key: K, run: Rc<F>) -> Subscription {
        let id = {
            let mut held = self.held.borrow_mut();
            let id = held.0;
            held.0 += 1;
            held.1.entry(key.clone()).or_default().push((id, run));
            id
        };
        let held = Rc::downgrade(&self.held);
        Subscription::new(move || {
            let Some(held) = held.upgrade() else {
                return;
            };
            let mut held = held.borrow_mut();
            let Some(runs) = held.1.get_mut(&key) else {
                return;
            };
            runs.retain(|(run, _)| *run != id);
            if runs.is_empty() {
                held.1.remove(&key);
            }
        })
    }

    pub(crate) fn of<'a>(&self, keys: impl IntoIterator<Item = &'a K>) -> Vec<Rc<F>> {
        let held = self.held.borrow();
        let mut runs: Vec<(u64, Rc<F>)> = keys
            .into_iter()
            .filter_map(|key| held.1.get(key))
            .flatten()
            .map(|(id, run)| (*id, Rc::clone(run)))
            .collect();
        runs.sort_by_key(|(id, _)| *id);
        runs.dedup_by_key(|(id, _)| *id);
        runs.into_iter().map(|(_, run)| run).collect()
    }

    pub(crate) fn all(&self) -> Vec<Rc<F>> {
        let keys: Vec<K> = self.held.borrow().1.keys().cloned().collect();
        self.of(&keys)
    }
}
