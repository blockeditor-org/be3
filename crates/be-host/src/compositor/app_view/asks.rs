//! Which child each editor instance asks take the keyboard, as it hears it
//! (`InstanceEvent::KeyboardAsked`, from a layout commit's `keyboard-ask`), kept until the child's
//! view acts on it or the instance's next commit withdraws it: the view may be built after the
//! event, in the layout that places the child. Nothing waits for the child's app
//! (decisions/keys_are_never_held.md).
//!
//! Each instance has its own trigger, so an ask reaches only the views of the children it places.

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::{ChildId, EditorInstanceId};
use beui::reactive::*;

type Key = (String, EditorInstanceId);

thread_local! {
    static ASKED: RefCell<HashMap<Key, ChildId>> = RefCell::new(HashMap::new());
    /// The children with a view built, by their placer, each with how many views show it.
    static VIEWED: RefCell<HashMap<(Key, ChildId), usize>> = RefCell::new(HashMap::new());
    /// One trigger per instance that has been tracked or heard. A trigger stays once made, as
    /// views that track it may outlive the instance; there is one per instance a session opened.
    /// It is never freed, so nothing frees it after the reactive arena at thread exit.
    static CHANGED: RefCell<HashMap<Key, Trigger>> = RefCell::new(HashMap::new());
}

fn trigger(key: &Key) -> Trigger {
    CHANGED.with_borrow_mut(|changed| {
        *changed.entry(key.clone()).or_insert_with(|| {
            let owned = OwnedTrigger::new();
            let trigger = owned.handle();
            std::mem::forget(owned);
            trigger
        })
    })
}

/// The instance `instance` of `app_id` asks that `child` take the keyboard, or withdrew its ask.
pub(crate) fn heard(app_id: String, instance: EditorInstanceId, child: Option<ChildId>) {
    let key = (app_id, instance);
    ASKED.with_borrow_mut(|asked| match child {
        Some(child) => asked.insert(key.clone(), child),
        None => asked.remove(&key),
    });
    trigger(&key).notify();
}

/// Whether the instance asks that `child` take the keyboard, tracking it.
pub(crate) fn asks(app_id: &str, instance: EditorInstanceId, child: ChildId) -> bool {
    trigger(&(app_id.to_owned(), instance)).track();
    ASKED.with_borrow(|asked| asked.get(&(app_id.to_owned(), instance)) == Some(&child))
}

/// The child's view acted on the ask: it no longer stands.
pub(crate) fn taken(app_id: &str, instance: EditorInstanceId) {
    let key = (app_id.to_owned(), instance);
    if ASKED.with_borrow_mut(|asked| asked.remove(&key)).is_some() {
        trigger(&key).notify();
    }
}

/// A view of `child`, which the instance placed, is built (`shown`) or went.
pub(crate) fn viewed(app_id: &str, instance: EditorInstanceId, child: ChildId, shown: bool) {
    let key = ((app_id.to_owned(), instance), child);
    VIEWED.with_borrow_mut(|viewed| match shown {
        true => *viewed.entry(key).or_default() += 1,
        false => {
            if let Some(count) = viewed.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    viewed.remove(&key);
                }
            }
        }
    });
}

/// Whether a view of `child`, which the instance placed, is built.
pub(crate) fn has_view(app_id: &str, instance: EditorInstanceId, child: ChildId) -> bool {
    VIEWED.with_borrow(|viewed| viewed.contains_key(&((app_id.to_owned(), instance), child)))
}
