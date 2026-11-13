//! Which child each editor instance gives the keyboard to, as it hears it
//! (`InstanceEvent::KeyboardGift`), kept until the child's view takes it: the view may be built
//! after the event, in the layout that places the child. be-host-runtime keeps the gift itself
//! and the keys held for it (its `instances/gift.rs`).
//!
//! Each instance has its own signal, so a gift reaches only the views that read that instance's:
//! its own views, which settle the keys they hold, and the views of the children it places.

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::{ChildId, EditorInstanceId};
use beui::reactive::*;

type Key = (String, EditorInstanceId);

thread_local! {
    static GIVEN: RefCell<HashMap<Key, ChildId>> = RefCell::new(HashMap::new());
    /// One signal per instance that has been read or heard. A signal stays once made, as views
    /// that read it may outlive the instance; there is one per instance a session opened. It is
    /// never freed, so nothing frees it after the reactive arena at thread exit.
    static CHANGED: RefCell<HashMap<Key, Signal<u64>>> = RefCell::new(HashMap::new());
}

fn signal(key: &Key) -> Signal<u64> {
    CHANGED.with_borrow_mut(|changed| {
        *changed.entry(key.clone()).or_insert_with(|| {
            let owned = OwnedSignal::new(0);
            let signal = owned.handle();
            std::mem::forget(owned);
            signal
        })
    })
}

/// The instance `instance` of `app_id` gives the keyboard to `child`, or none.
pub(crate) fn heard(app_id: String, instance: EditorInstanceId, child: Option<ChildId>) {
    let key = (app_id, instance);
    GIVEN.with_borrow_mut(|given| match child {
        Some(child) => given.insert(key.clone(), child),
        None => given.remove(&key),
    });
    signal(&key).update(|changed| *changed += 1);
}

/// Moves whenever the instance's gift is heard or taken, and whenever it ends: the views that
/// hold keys for it settle them then.
pub(super) fn track(app_id: &str, instance: EditorInstanceId) {
    signal(&(app_id.to_owned(), instance)).get();
}

/// Whether the instance gives the keyboard to `child`, tracking it.
pub(crate) fn gives(app_id: &str, instance: EditorInstanceId, child: ChildId) -> bool {
    track(app_id, instance);
    GIVEN.with_borrow(|given| given.get(&(app_id.to_owned(), instance)) == Some(&child))
}

/// The child's view acted on the gift: it is no longer waiting to be taken.
pub(crate) fn taken(app_id: &str, instance: EditorInstanceId) {
    let key = (app_id.to_owned(), instance);
    if GIVEN.with_borrow_mut(|given| given.remove(&key)).is_some() {
        signal(&key).update(|changed| *changed += 1);
    }
}
