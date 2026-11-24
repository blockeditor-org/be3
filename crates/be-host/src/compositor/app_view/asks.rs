//! Which child each editor instance asks take the keyboard, as it hears it
//! (`InstanceEvent::KeyboardAsked`, from a layout commit's `keyboard-ask`), kept until the child's
//! view acts on it or the instance's next commit withdraws it: the view may be built after the
//! event, in the layout that places the child. Nothing waits for the child's app
//! (decisions/keys_are_never_held.md).
//!
//! Each instance has its own trigger, so an ask reaches only the views of the children it places.

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::{ChildId, EditorInstanceId, ViewId};
use beui::reactive::*;

type Key = (String, EditorInstanceId);

thread_local! {
    static ASKED: RefCell<HashMap<Key, ChildId>> = RefCell::new(HashMap::new());
    /// The views up of the children each instance placed: the child, its instance and the view.
    static VIEWED: RefCell<HashMap<Key, Vec<(ChildId, EditorInstanceId, ViewId)>>> =
        RefCell::new(HashMap::new());
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

/// A view of `child`, which the instance placed, came up (`shown`) or went: `region` of the
/// child's instance `own`.
pub(crate) fn viewed(
    app_id: &str,
    instance: EditorInstanceId,
    child: ChildId,
    (own, region): (EditorInstanceId, ViewId),
    shown: bool,
) {
    let key = (app_id.to_owned(), instance);
    VIEWED.with_borrow_mut(|viewed| {
        let views = viewed.entry(key.clone()).or_default();
        match shown {
            true => views.push((child, own, region)),
            false => {
                if let Some(at) = views.iter().position(|view| *view == (child, own, region)) {
                    views.swap_remove(at);
                }
            }
        }
        if views.is_empty() {
            viewed.remove(&key);
        }
    });
    trigger(&key).notify();
}

/// Whether a view of `child`, which the instance placed, is up.
pub(crate) fn has_view(app_id: &str, instance: EditorInstanceId, child: ChildId) -> bool {
    VIEWED.with_borrow(|viewed| {
        viewed
            .get(&(app_id.to_owned(), instance))
            .is_some_and(|views| views.iter().any(|view| view.0 == child))
    })
}

/// Whether the view `region` of the instance `own`, up as the instance's child `child`, is the one
/// to act on the instance's ask, tracking it. The asked child takes the keyboard in its content:
/// where the placer's dock shows that in a body of its own (`content`, a view of the same
/// instance up under another of the placer's children), that view acts, not the child's own
/// view or the bars docked with it; otherwise the asked child's own view does.
pub(crate) fn acts(
    app_id: &str,
    instance: EditorInstanceId,
    child: ChildId,
    (own, region, content): (EditorInstanceId, ViewId, ViewId),
) -> bool {
    let key = (app_id.to_owned(), instance);
    trigger(&key).track();
    let Some(asked) = ASKED.with_borrow(|asked| asked.get(&key).copied()) else {
        return false;
    };
    VIEWED.with_borrow(|viewed| {
        let views = viewed.get(&key).map(Vec::as_slice).unwrap_or_default();
        let asked_own = views.iter().any(|view| view.0 == asked && view.1 == own);
        if !asked_own {
            return false;
        }
        match views.iter().any(|view| view.1 == own && view.2 == content) {
            true => region == content,
            false => child == asked,
        }
    })
}
