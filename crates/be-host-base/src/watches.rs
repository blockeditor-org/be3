use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::Arc,
};

use be_sdk_api::{
    Watched,
    ops::{MethodId, Started},
};

pub trait WatchHost {
    fn published(&self, open: MethodId, value: Arc<Vec<u8>>);
    fn watched(&self, open: MethodId) -> bool;
    fn take_calls(&self, accept: &mut dyn FnMut(&Started) -> bool);
}

thread_local! {
    static PUBLISHED: RefCell<HashMap<MethodId, Arc<Vec<u8>>>> = RefCell::new(HashMap::new());
    static HOST: Cell<Option<&'static dyn WatchHost>> = const { Cell::new(None) };
    static OPENINGS: RefCell<HashMap<MethodId, u64>> = RefCell::new(HashMap::new());
}

pub fn serve(host: &'static dyn WatchHost) {
    HOST.with(|held| held.set(Some(host)));
}

pub fn publish<W: Watched>(value: &W::Value) {
    let bytes = be_sdk_api::ops::encode(value);
    let value = PUBLISHED.with(|published| {
        let mut published = published.borrow_mut();
        if published
            .get(&W::OPEN)
            .is_some_and(|published| **published == bytes)
        {
            return None;
        }
        let value = Arc::new(bytes);
        published.insert(W::OPEN, Arc::clone(&value));
        Some(value)
    });
    if let Some(value) = value
        && let Some(host) = HOST.with(Cell::get)
    {
        host.published(W::OPEN, value);
    }
}

pub fn published() -> Vec<(MethodId, Arc<Vec<u8>>)> {
    PUBLISHED.with(|published| {
        published
            .borrow()
            .iter()
            .map(|(open, value)| (*open, Arc::clone(value)))
            .collect()
    })
}

/// Counts a watch opened by an editor; the host that serves watches calls it.
pub fn opened(open: MethodId) {
    OPENINGS.with(|openings| *openings.borrow_mut().entry(open).or_default() += 1);
}

/// How many watches of `W` have been opened, for a watch whose every opening starts something
/// (an input capture), which `watched` alone cannot tell from one kept open.
pub fn openings<W: Watched>() -> u64 {
    OPENINGS.with(|openings| openings.borrow().get(&W::OPEN).copied().unwrap_or_default())
}

pub fn watched<W: Watched>() -> bool {
    HOST.with(Cell::get)
        .is_some_and(|host| host.watched(W::OPEN))
}

pub fn take_calls<C>(accept: impl Fn(&Started) -> Option<C>) -> Vec<C> {
    let mut taken = Vec::new();
    if let Some(host) = HOST.with(Cell::get) {
        host.take_calls(&mut |called| match accept(called) {
            Some(call) => {
                taken.push(call);
                true
            }
            None => false,
        });
    }
    taken
}
