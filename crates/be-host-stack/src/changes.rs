//! What the stack tells the host changed (`ChangeWatch`): pushed into each watcher as it
//! happens, never polled from the stack's state. Each watcher holds what changed since it last
//! took it, and the first change after that wakes the host, so it hears once a frame whatever
//! the number of changes.
//!
//! The watchers are a registry rather than letters in the host's mailbox: several consumers
//! (be-host's core, each app runtime, the compositor, the input remapper) each want every
//! change, and a mailbox delivers a letter once, to one receiver. It is process-wide, so stacks
//! that run side by side in tests wake each other's watchers, which only costs a look.

use be_wasm_threads::Mutex;
use std::sync::{Arc, PoisonError, Weak};
use uuid::Uuid;

/// What changed since a watcher last heard.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Changes {
    /// The graph mirror: a query may answer differently, and so may what apps are told of a
    /// block (`BlockInfo::out_of_reach` changes with the connection).
    pub graph: bool,
    pub contacts: bool,
    /// The account's storage (`storage_usage`).
    pub storage: bool,
    /// Which blocks are stale, have conflicts, or have a kind this host can read
    /// (`Modules::with_kinds_changed`).
    pub stale: bool,
    /// The answers to searches (`searched`).
    pub search: bool,
    /// The account's notification modes (`notify_modes`).
    pub notify_modes: bool,
    /// The blocks whose content, or what is shown of it, changed.
    pub blocks: be_lib_stable_hash::HashSet<Uuid>,
    /// Every block's content may have changed: the stack started or stopped.
    pub every_block: bool,
}

impl Changes {
    /// Everything: what a new watcher hears first, and every watcher when the stack starts or
    /// stops.
    pub fn everything() -> Self {
        Self {
            graph: true,
            contacts: true,
            storage: true,
            stale: true,
            search: true,
            notify_modes: true,
            blocks: Default::default(),
            every_block: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn merge(&mut self, other: Changes) {
        self.graph |= other.graph;
        self.contacts |= other.contacts;
        self.storage |= other.storage;
        self.stale |= other.stale;
        self.search |= other.search;
        self.notify_modes |= other.notify_modes;
        self.blocks.extend(other.blocks);
        self.every_block |= other.every_block;
    }

    /// Whether `block`'s content may have changed.
    pub fn block(&self, block: Uuid) -> bool {
        self.every_block || self.blocks.contains(&block)
    }
}

static WATCHERS: Mutex<Vec<Weak<Mutex<Changes>>>> = Mutex::new(Vec::new());

/// Holds what the stack changed until taken, which the stack pushes into it as it happens.
/// The first change after a take wakes the host, which takes it in its next frame. Dropping it
/// ends the watch.
pub struct ChangeWatch {
    held: Arc<Mutex<Changes>>,
}

impl ChangeWatch {
    /// A new watch, which first tells of everything: nothing is heard of what was there before.
    pub fn new() -> Self {
        let held = Arc::new(Mutex::new(Changes::everything()));
        let mut watchers = WATCHERS.lock().unwrap_or_else(PoisonError::into_inner);
        watchers.retain(|watcher| watcher.strong_count() > 0);
        watchers.push(Arc::downgrade(&held));
        Self { held }
    }

    /// What changed since the last take.
    pub fn take(&self) -> Changes {
        std::mem::take(&mut *self.held.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Default for ChangeWatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Tells every watcher of `changes`.
pub(crate) fn post(changes: Changes) {
    if changes.is_empty() {
        return;
    }
    let mut woken = false;
    let mut watchers = WATCHERS.lock().unwrap_or_else(PoisonError::into_inner);
    watchers.retain(|watcher| {
        let Some(held) = watcher.upgrade() else {
            return false;
        };
        let mut held = held.lock().unwrap_or_else(PoisonError::into_inner);
        woken |= held.is_empty();
        held.merge(changes.clone());
        true
    });
    drop(watchers);
    if woken {
        be_host_base::host::wake();
    }
}

pub(crate) fn graph() {
    post(Changes {
        graph: true,
        ..Changes::default()
    });
}

pub(crate) fn stale() {
    post(Changes {
        stale: true,
        ..Changes::default()
    });
}

pub(crate) fn block(block: Uuid) {
    post(Changes {
        blocks: [block].into_iter().collect(),
        ..Changes::default()
    });
}

/// The stack started or stopped: every watcher reads everything anew.
pub(crate) fn everything() {
    post(Changes::everything());
}
