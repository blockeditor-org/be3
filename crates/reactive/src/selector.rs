use stable_hash::HashMap;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use crate::arena::{self, Key};
use crate::computation::create_effect;
use crate::memo::{Memo, create_memo};
use crate::runtime::{check_tracked_read, tracking};
use crate::scope::on_cleanup;
use crate::signal::{Get, Source, track};

struct Watched {
    source: Rc<Source>,
    watchers: usize,
}

type Keys<K> = Rc<RefCell<HashMap<K, Watched>>>;

pub struct Selector<K> {
    value: Memo<K>,
    keys: Key,
}

pub fn create_selector<K: Clone + Eq + Hash + 'static>(
    mut source: impl FnMut() -> K + 'static,
) -> Selector<K> {
    let keys: Keys<K> = Rc::new(RefCell::new(HashMap::default()));
    let value = {
        let keys = keys.clone();
        let mut previous: Option<K> = None;
        create_memo(move || {
            let next = source();
            if let Some(previous) = previous.replace(next.clone())
                && previous != next
            {
                notify(&keys, &previous);
                notify(&keys, &next);
            }
            next
        })
    };
    create_effect(move || value.with(|_| ()));
    Selector {
        value,
        keys: arena::insert_owned(keys),
    }
}

fn notify<K: Eq + Hash>(keys: &Keys<K>, key: &K) {
    let source = keys.borrow().get(key).map(|watched| watched.source.clone());
    if let Some(source) = source {
        source.changed();
    }
}

impl<K> Clone for Selector<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Selector<K> {}

impl<K: Clone + Eq + Hash + 'static> Selector<K> {
    #[track_caller]
    pub fn is_selected(&self, key: &K) -> bool {
        check_tracked_read::<Self>();
        let selected = self.value.with_untracked(|current| current == key);
        self.watch(key);
        selected
    }

    pub fn memo(&self, key: K) -> Memo<bool> {
        let selector = *self;
        create_memo(move || selector.is_selected(&key))
    }

    fn keys(&self) -> Keys<K> {
        arena::get(self.keys).expect("selector used after the scope that owns it was disposed")
    }

    fn watch(&self, key: &K) {
        if !tracking() {
            return;
        }
        let keys = self.keys();
        let source = {
            let mut keys = keys.borrow_mut();
            let watched = keys.entry(key.clone()).or_insert_with(|| Watched {
                source: Rc::new(Source::default()),
                watchers: 0,
            });
            watched.watchers += 1;
            watched.source.clone()
        };
        track(source);
        let key = key.clone();
        on_cleanup(move || {
            let mut keys = keys.borrow_mut();
            let Some(watched) = keys.get_mut(&key) else {
                return;
            };
            watched.watchers -= 1;
            if watched.watchers == 0 {
                keys.remove(&key);
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn watched_keys(&self) -> usize {
        self.keys().borrow().len()
    }
}
