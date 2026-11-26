//! What part writers sealed that nothing may name yet (guides/blocks/client.md, "Unnamed
//! parts"), kept for every vault over one store.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};
use uuid::Uuid;

use crate::{StoreError, hash::Hash, manifest::PartRef, store::ObjectStore};

/// Where the vaults over one store keep, across restarts, the objects their part writers sealed
/// that the store did not hold, until a collection (`Vault::collect`) finds them named or
/// removes them. be-block-client keeps them in the refs database, one ledger for every scope.
pub trait Ledger: Send + Sync {
    /// Every object noted and not yet forgotten.
    fn noted(&self) -> Result<Vec<(Hash, Noted)>, StoreError>;

    /// Notes `object` before it is put.
    fn note(&self, object: Hash, noted: Noted) -> Result<(), StoreError>;

    fn forget(&self, objects: &[Hash]) -> Result<(), StoreError>;
}

/// Who sealed a noted object, and for which block.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Noted {
    /// The block whose key sealed it.
    pub block: Uuid,
    /// The vault whose part writer sealed it (`Vault::with_unnamed`): only that owner's
    /// collections look at it, since only its commits and edits are walked.
    pub owner: Hash,
}

/// What the part writers of every vault over one store sealed that nothing may name yet. The
/// vaults share it (a clone shares), each as its own owner (`Vault::with_unnamed`), so that an
/// object one owner noted is claimed once another puts, looks for or reads it, and pins and
/// collections exclude each other across them all.
#[derive(Clone, Default)]
pub struct Unnamed(Arc<Mutex<Notes>>);

impl Unnamed {
    /// Keeps what is noted in `ledger`, starting from what it noted before, so a collection
    /// after a restart still finds a part a stopped writer left.
    pub fn with_ledger(ledger: Arc<dyn Ledger>) -> Result<Self, StoreError> {
        let notes = Notes {
            noted: ledger.noted()?.into_iter().collect(),
            ledger: Some(ledger),
            ..Notes::default()
        };
        Ok(Self(Arc::new(Mutex::new(notes))))
    }

    /// A handle that keeps nothing alive: once every vault and pin over it goes, what it noted
    /// is only in its ledger.
    pub fn downgrade(&self) -> WeakUnnamed {
        WeakUnnamed(Arc::downgrade(&self.0))
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, Notes> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// An `Unnamed` while anything else holds it (`Unnamed::downgrade`).
#[derive(Clone)]
pub struct WeakUnnamed(Weak<Mutex<Notes>>);

impl WeakUnnamed {
    pub fn upgrade(&self) -> Option<Unnamed> {
        self.0.upgrade().map(Unnamed)
    }
}

#[derive(Default)]
pub(super) struct Notes {
    /// Objects a part writer put that the store lacked: nothing named them then.
    pub noted: HashMap<Hash, Noted>,
    /// How many open part writers, or parts they finished that are still held (`Written`), name
    /// each object: never collected while named so.
    pub pinned: HashMap<Hash, usize>,
    /// How many store accesses (`VaultStore`) are under way: a collection removes nothing
    /// while any is, since one may have found an object it has not yet claimed.
    pub accessing: usize,
    pub ledger: Option<Arc<dyn Ledger>>,
}

/// How an object noted unnamed came to be reached other than by the writer that noted it.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Claim {
    /// Put by anything but a part writer, which names it.
    Put,
    /// Looked for by its presence (`holds_all`, `missing_objects`), by what may name it now.
    Looked,
    /// Found or read through the store. Only another owner's claims: the owner's own merge
    /// reads back what it merged, its host collects only while no read is under way, and its
    /// collections read what they walk.
    Read,
}

impl Notes {
    /// Notes no more `objects`, in the ledger and then here: a ledger that fails to forget
    /// leaves them noted in both.
    pub fn forget(&mut self, objects: &[Hash]) -> Result<(), StoreError> {
        if objects.is_empty() {
            return Ok(());
        }
        if let Some(ledger) = &self.ledger {
            ledger.forget(objects)?;
        }
        for object in objects {
            self.noted.remove(object);
        }
        Ok(())
    }

    /// `objects`, reached by `owner` as `how` says, may be named by what reached them: no
    /// longer noted, here and in the ledger at once, so a crash before the pins go does not
    /// leave them noted for a collection after the restart. One the owner itself looked for
    /// stays noted while pinned: its writer, or whoever holds its part, reads it back as it
    /// works, and the pin keeps it meanwhile.
    pub fn claim(&mut self, objects: &[Hash], owner: Hash, how: Claim) -> Result<(), StoreError> {
        if self.noted.is_empty() {
            return Ok(());
        }
        let mut forgotten = Vec::new();
        for object in objects {
            let Some(noted) = self.noted.get(object) else {
                continue;
            };
            let own = noted.owner == owner;
            if own && how == Claim::Read {
                continue;
            }
            if !(own && how == Claim::Looked && self.pinned.contains_key(object)) {
                forgotten.push(*object);
            }
        }
        self.forget(&forgotten)
    }
}

/// The store a vault reads and writes through (`Vault::store`), as one owner of the objects
/// noted unnamed (`Unnamed`): each access counts as under way, so no collection removes what
/// it found, and claims what it put, or found of another owner's, once done.
pub struct VaultStore<S> {
    pub(super) inner: S,
    pub(super) unnamed: Unnamed,
    pub(super) owner: Hash,
}

/// The store itself, for what it has besides `ObjectStore` (counts, a clone for another
/// vault): reading or writing through it bypasses the claims.
impl<S> std::ops::Deref for VaultStore<S> {
    type Target = S;

    fn deref(&self) -> &S {
        &self.inner
    }
}

impl<S: ObjectStore> VaultStore<S> {
    fn access<T>(
        &self,
        how: Claim,
        access: impl FnOnce(&S) -> Result<T, StoreError>,
        object: impl FnOnce(&T) -> Hash,
    ) -> Result<T, StoreError> {
        self.unnamed.lock().accessing += 1;
        let accessing = Accessing(&self.unnamed);
        let answer = access(&self.inner)?;
        let object = object(&answer);
        let mut notes = self.unnamed.lock();
        std::mem::forget(accessing);
        notes.accessing -= 1;
        notes.claim(&[object], self.owner, how)?;
        Ok(answer)
    }
}

/// An access under way, counted off if it fails.
struct Accessing<'a>(&'a Unnamed);

impl Drop for Accessing<'_> {
    fn drop(&mut self) {
        self.0.lock().accessing -= 1;
    }
}

impl<S: ObjectStore> ObjectStore for VaultStore<S> {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        self.access(Claim::Put, |store| store.put(bytes), |hash| *hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        self.access(Claim::Read, |store| store.get(hash), |_| hash)
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        self.access(Claim::Read, |store| store.has(hash), |_| hash)
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        self.inner.remove(hash)
    }

    fn sync(&self) -> Result<(), StoreError> {
        self.inner.sync()
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        self.access(Claim::Read, |store| store.len(hash), |_| hash)
    }
    fn identity(&self) -> Option<Vec<u8>> {
        self.inner.identity()
    }
}

/// The objects a part writer sealed, kept from collection while this lives: the writer's, then
/// its finished part's (`Written`), until whoever holds it names the part in a commit or lets
/// go of it.
pub struct Pins {
    pub(super) unnamed: Unnamed,
    pub(super) objects: Vec<Hash>,
}

impl Drop for Pins {
    fn drop(&mut self) {
        let mut notes = self.unnamed.lock();
        for object in self.objects.drain(..) {
            if let Entry::Occupied(mut entry) = notes.pinned.entry(object) {
                *entry.get_mut() -= 1;
                if *entry.get() == 0 {
                    entry.remove();
                }
            }
        }
    }
}

/// A part a part writer finished (`PartWriter::finish`): its objects stay out of a collection's
/// reach while this is held, so hold it until the part is named (a commit sealed into the outbox
/// with it) or is let go of. It borrows nothing, so a replica can hold it as long as it needs.
pub struct Written {
    pub(super) part: Option<PartRef>,
    pub(super) _pins: Option<Pins>,
}

impl std::fmt::Debug for Written {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Written").field(&self.part).finish()
    }
}

impl Written {
    /// The part, or None when no byte was written.
    pub fn part(&self) -> Option<PartRef> {
        self.part
    }
}
