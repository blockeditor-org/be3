//! What the stack tells the host changed (`ChangeWatch`): pushed through the host's mailbox
//! as it happens, never polled from the stack's state. Each watcher holds what changed since its
//! last letter, and the first change after that posts it one, so it hears once a frame whatever
//! the number of changes.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, PoisonError, Weak},
};

use be_host_base::host::{Mailbox, Post};
use be_wasm_threads::Mutex;
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
    /// (`be_block_type_host::Host::kinds_changed`).
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

struct Watcher {
    alive: Weak<()>,
    held: Arc<Mutex<Changes>>,
    post: Post<()>,
}

static WATCHERS: Mutex<Vec<Watcher>> = Mutex::new(Vec::new());

/// Hears what the stack changed, through a mailbox on the thread that made it, delivered at the
/// start of each frame (`be_host_base::host::begin_frame`), and holds it until taken. Dropping
/// it ends the watch.
pub struct ChangeWatch {
    heard: Rc<RefCell<Changes>>,
    _alive: Arc<()>,
    _mailbox: Mailbox,
}

impl ChangeWatch {
    /// A new watch, which first tells of everything: nothing is heard of what was there before.
    pub fn new() -> Self {
        let heard = Rc::new(RefCell::new(Changes::everything()));
        let held = Arc::new(Mutex::new(Changes::default()));
        let (mailbox, post) = be_host_base::host::mailbox({
            let (heard, held) = (Rc::clone(&heard), Arc::clone(&held));
            move |()| {
                let changes =
                    std::mem::take(&mut *held.lock().unwrap_or_else(PoisonError::into_inner));
                heard.borrow_mut().merge(changes);
            }
        });
        let alive = Arc::new(());
        let mut watchers = WATCHERS.lock().unwrap_or_else(PoisonError::into_inner);
        watchers.retain(|watcher| watcher.alive.strong_count() > 0);
        watchers.push(Watcher {
            alive: Arc::downgrade(&alive),
            held,
            post,
        });
        Self {
            heard,
            _alive: alive,
            _mailbox: mailbox,
        }
    }

    /// What changed since the last take.
    pub fn take(&self) -> Changes {
        std::mem::take(&mut *self.heard.borrow_mut())
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
    let mut watchers = WATCHERS.lock().unwrap_or_else(PoisonError::into_inner);
    watchers.retain(|watcher| watcher.alive.strong_count() > 0);
    for watcher in watchers.iter() {
        let mut held = watcher.held.lock().unwrap_or_else(PoisonError::into_inner);
        let first = held.is_empty();
        held.merge(changes.clone());
        if first {
            watcher.post.send(());
        }
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
