//! Which child each editor instance gives the keyboard to, as it hears it
//! (`InstanceEvent::KeyboardGift`), kept until the child's view takes it: the view may be built
//! after the event, in the layout that places the child. be-host-runtime keeps the gift itself
//! and the keys held for it (its `instances/gift.rs`).

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::{ChildId, EditorInstanceId};
use beui::reactive::*;

thread_local! {
    static GIVEN: RefCell<HashMap<(String, EditorInstanceId), ChildId>> =
        RefCell::new(HashMap::new());
    static CHANGED: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
}

pub(crate) fn install() {
    let changed = create_signal(0u64);
    CHANGED.with(|held| *held.borrow_mut() = Some(changed));
}

/// The instance `instance` of `app_id` gives the keyboard to `child`, or none.
pub(crate) fn heard(app_id: String, instance: EditorInstanceId, child: Option<ChildId>) {
    GIVEN.with_borrow_mut(|given| match child {
        Some(child) => given.insert((app_id, instance), child),
        None => given.remove(&(app_id, instance)),
    });
    changed();
}

fn changed() {
    if let Some(changed) = CHANGED.with(|held| *held.borrow()) {
        changed.update(|changed| *changed += 1);
    }
}

/// Moves whenever a gift is heard or taken, and whenever one ends: the views that hold keys for
/// one settle them then.
pub(super) fn track() {
    if let Some(changed) = CHANGED.with(|held| *held.borrow()) {
        changed.get();
    }
}

/// Whether the instance gives the keyboard to `child`, tracking it.
pub(crate) fn gives(app_id: &str, instance: EditorInstanceId, child: ChildId) -> bool {
    track();
    GIVEN.with_borrow(|given| given.get(&(app_id.to_owned(), instance)) == Some(&child))
}

/// The child's view acted on the gift: it is no longer waiting to be taken.
pub(crate) fn taken(app_id: &str, instance: EditorInstanceId) {
    if GIVEN
        .with_borrow_mut(|given| given.remove(&(app_id.to_owned(), instance)))
        .is_some()
    {
        changed();
    }
}
