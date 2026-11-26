use be_block_keys::aead;
use rand::TryRngCore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

mod unnamed;

pub use unnamed::{Ledger, Noted, Pins, Unnamed, VaultStore, Written};
use unnamed::{COLLECTING, Claim};

use crate::{
    StoreError,
    chunker::{ChunkerConfig, Splitter},
    hash::Hash,
    manifest::{
        ChunkRef, LoadedPart, Manifest, Node, NodeRef, Part, PartRef, Stored, decode_value,
        encode_value,
    },
    store::ObjectStore,
};

pub const NONCE_LEN: usize = aead::NONCE_LEN;
pub const TAG: &[u8; 4] = b"be3K";
const HEADER_LEN: usize = TAG.len() + 16;
const GROUP_MINIMUM: usize = 4;
const GROUP_AVERAGE: u16 = 64;
const GROUP_MAXIMUM: usize = 256;

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentKey([u8; 32]);

impl ContentKey {
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .expect("the operating system has entropy");
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// `plain` sealed as key `id` of `block` (`be_block_keys::aead`): the header (`TAG` and
    /// `id`), the nonce, then the ciphertext, which authenticates the header and `block` too,
    /// so a header relabelled to another key, or a key placed in another block, opens nothing.
    /// Content, which deduplicates, derives its nonce from `block` and `plain` (`derived`);
    /// anything else draws it (decisions/deterministic_seal_nonce.md).
    fn seal(&self, id: Uuid, block: Uuid, plain: &[u8], derived: bool) -> Vec<u8> {
        let nonce = match derived {
            true => aead::derived_nonce(self.as_bytes(), block.as_bytes(), plain),
            false => aead::random_nonce(),
        };
        let mut sealed = Vec::with_capacity(HEADER_LEN + NONCE_LEN + plain.len() + 16);
        sealed.extend_from_slice(TAG);
        sealed.extend_from_slice(id.as_bytes());
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&aead::seal(
            self.as_bytes(),
            &nonce,
            &associated(id, block),
            plain,
        ));
        sealed
    }

    fn open_body(&self, id: Uuid, block: Uuid, body: &[u8]) -> Result<Vec<u8>, StoreError> {
        let (nonce, ciphertext) = body
            .split_at_checked(NONCE_LEN)
            .ok_or(StoreError::Corrupt)?;
        let nonce: aead::Nonce = nonce.try_into().map_err(|_| StoreError::Corrupt)?;
        aead::open(self.as_bytes(), &nonce, &associated(id, block), ciphertext)
            .map_err(|_| StoreError::Corrupt)
    }
}

impl std::fmt::Debug for ContentKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContentKey(..)")
    }
}

/// What a seal authenticates beside its plaintext: its header and the block its key is of.
fn associated(id: Uuid, block: Uuid) -> [u8; HEADER_LEN + 16] {
    let mut associated = [0u8; HEADER_LEN + 16];
    associated[..TAG.len()].copy_from_slice(TAG);
    associated[TAG.len()..HEADER_LEN].copy_from_slice(id.as_bytes());
    associated[HEADER_LEN..].copy_from_slice(block.as_bytes());
    associated
}

pub fn sealed_key(sealed: &[u8]) -> Result<Uuid, StoreError> {
    let rest = sealed.strip_prefix(TAG).ok_or(StoreError::Corrupt)?;
    let id: [u8; 16] = rest
        .get(..16)
        .and_then(|id| id.try_into().ok())
        .ok_or(StoreError::Corrupt)?;
    Ok(Uuid::from_bytes(id))
}

#[derive(Clone, Default)]
/// Content keys by id, each with the block it is a key of, which every seal under it binds.
pub struct Keyring {
    keys: Arc<RwLock<HashMap<Uuid, (ContentKey, Uuid)>>>,
}

impl Keyring {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps `key` as key `id` of `block`.
    pub fn insert(&self, id: Uuid, key: ContentKey, block: Uuid) {
        self.keys
            .write()
            .expect("the keyring is never poisoned")
            .insert(id, (key, block));
    }

    pub fn get(&self, id: Uuid) -> Option<ContentKey> {
        self.held(id).map(|(key, _)| key)
    }

    /// Key `id` and the block it is a key of.
    fn held(&self, id: Uuid) -> Option<(ContentKey, Uuid)> {
        self.keys
            .read()
            .expect("the keyring is never poisoned")
            .get(&id)
            .copied()
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.keys
            .read()
            .expect("the keyring is never poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `plain` sealed under `key` with a nonce drawn at random, for what never deduplicates.
    pub fn seal(&self, key: Uuid, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        let (sealing, block) = self.held(key).ok_or(StoreError::MissingKey(key))?;
        Ok(sealing.seal(key, block, plain, false))
    }

    /// `plain` sealed under `key` with a nonce derived from it, so equal content under one key
    /// seals to one object.
    fn seal_content(&self, key: Uuid, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        let (sealing, block) = self.held(key).ok_or(StoreError::MissingKey(key))?;
        Ok(sealing.seal(key, block, plain, true))
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, StoreError> {
        let id = sealed_key(sealed)?;
        let (key, block) = self.held(id).ok_or(StoreError::MissingKey(id))?;
        key.open_body(id, block, &sealed[HEADER_LEN..])
    }
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Keyring({} keys)", self.len())
    }
}

/// Seals with a key it names by id from its `Keyring`, and opens by the id the sealed bytes
/// carry. The nonce is derived from the key and the plaintext, so identical content
/// deduplicates under one key and shares nothing across keys
/// (decisions/deterministic_seal_nonce.md).
pub struct Vault<S> {
    store: VaultStore<S>,
    keys: Keyring,
    chunker: ChunkerConfig,
    sealed: RwLock<HashMap<Hash, Uuid>>,
}

/// One part's bytes being written a piece at a time (`Vault::part_writer`): each chunk is sealed
/// and put as soon as the bytes end it, so it holds at most one chunk's bytes and the refs of the
/// chunks so far. What it seals is pinned (`Pins`) from the moment it is sealed.
pub struct PartWriter<'a, S> {
    vault: &'a Vault<S>,
    key: Uuid,
    sealing: (ContentKey, Uuid),
    splitter: Splitter,
    chunks: Vec<PartRef>,
    /// None for the writer inside `Vault::write`, whose caller names what it seals at once.
    pins: Option<Pins>,
}

impl<'a, S: ObjectStore> PartWriter<'a, S> {
    /// Appends `bytes` to the part.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        let Self {
            vault,
            key,
            sealing,
            splitter,
            chunks,
            pins,
        } = self;
        splitter.push(bytes, |chunk| {
            chunks.push(vault.put_chunk(*key, sealing, chunk, pins.as_mut())?);
            Ok(())
        })
    }

    /// The bytes written that no chunk sealed yet holds.
    pub fn held(&self) -> usize {
        self.splitter.held()
    }

    /// Seals the rest and answers the part, pinned while the answer is held.
    pub fn finish(self) -> Result<Written, StoreError> {
        let Self {
            vault,
            key,
            sealing,
            splitter,
            mut chunks,
            mut pins,
        } = self;
        splitter.finish(|chunk| {
            chunks.push(vault.put_chunk(key, &sealing, chunk, pins.as_mut())?);
            Ok::<_, StoreError>(())
        })?;
        let part = vault.group(key, &sealing, chunks, pins.as_mut())?;
        Ok(Written { part, _pins: pins })
    }
}

impl<S: ObjectStore> Vault<S> {
    pub fn new(store: S, keys: Keyring) -> Self {
        Self {
            store: VaultStore {
                inner: store,
                unnamed: Unnamed::default(),
                owner: Hash::from_bytes([0; 32]),
            },
            keys,
            chunker: ChunkerConfig::DEFAULT,
            sealed: RwLock::default(),
        }
    }

    /// Keeps what its part writers seal unnamed in `unnamed`, which every vault over this
    /// store shares, as `owner`: its collections look only at what its own writers noted, and
    /// what it puts, looks for or reads of another owner's is claimed (`Unnamed`).
    pub fn with_unnamed(mut self, unnamed: Unnamed, owner: Hash) -> Self {
        self.store.unnamed = unnamed;
        self.store.owner = owner;
        self
    }

    pub fn with_chunker(mut self, chunker: ChunkerConfig) -> Self {
        self.chunker = chunker;
        self
    }

    /// The store, through which what is read and written claims the objects noted unnamed
    /// that it reaches (`VaultStore`).
    pub fn store(&self) -> &VaultStore<S> {
        &self.store
    }

    pub fn keys(&self) -> &Keyring {
        &self.keys
    }

    pub fn seal(&self, key: Uuid, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        self.keys.seal(key, plain)
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, StoreError> {
        self.keys.open(sealed)
    }

    /// Seals the new parts of `parts` and writes the manifest tree over them, keeping every
    /// `Part::Reused` as it was.
    pub fn write<'a>(
        &self,
        key: Uuid,
        content_type: Uuid,
        parts: impl Into<Part<'a>>,
    ) -> Result<Manifest, StoreError> {
        let sealing = self.keys.held(key).ok_or(StoreError::MissingKey(key))?;
        let root = self.write_part(key, &sealing, &parts.into())?;
        Ok(Manifest { content_type, root })
    }

    /// A writer of one part's bytes, sealed under `key` a chunk at a time as they come: what
    /// `write` makes of a `Part::Bytes` of the same bytes, without holding them all.
    pub fn part_writer(&self, key: Uuid) -> Result<PartWriter<'_, S>, StoreError> {
        let sealing = self.keys.held(key).ok_or(StoreError::MissingKey(key))?;
        let pins = Pins {
            unnamed: self.store.unnamed.clone(),
            objects: Vec::new(),
        };
        Ok(self.writer(key, sealing, Some(pins)))
    }

    fn writer<'a>(
        &'a self,
        key: Uuid,
        sealing: (ContentKey, Uuid),
        pins: Option<Pins>,
    ) -> PartWriter<'a, S> {
        PartWriter {
            vault: self,
            key,
            sealing,
            splitter: Splitter::new(self.chunker),
            chunks: Vec::new(),
            pins,
        }
    }

    fn put_chunk(
        &self,
        key: Uuid,
        sealing: &(ContentKey, Uuid),
        chunk: &[u8],
        pins: Option<&mut Pins>,
    ) -> Result<PartRef, StoreError> {
        let length = u32::try_from(chunk.len()).map_err(|_| StoreError::ChunkTooLarge)?;
        let sealed = sealing.0.seal(key, sealing.1, chunk, true);
        let hash = self.put_written(key, sealing.1, &sealed, pins)?;
        Ok(PartRef::Chunk(ChunkRef { hash, length }))
    }

    /// Puts what a writer sealed. A part writer's (`pins`) is pinned first, and noted unnamed
    /// when the store lacks it; one another owner noted is claimed, since this owner's writer
    /// may come to name it. Anything else is named by whoever put it.
    fn put_written(
        &self,
        key: Uuid,
        block: Uuid,
        sealed: &[u8],
        pins: Option<&mut Pins>,
    ) -> Result<Hash, StoreError> {
        let Some(pins) = pins else {
            return self.put_sealed(key, sealed);
        };
        let hash = Hash::of(sealed);
        let owner = self.store.owner;
        {
            let mut notes = self.store.unnamed.lock();
            *notes.pinned.entry(hash).or_default() += 1;
            pins.objects.push(hash);
            match notes.noted.get(&hash) {
                Some(noted) if noted.owner != owner => notes.claim(&[hash], owner, Claim::Put)?,
                Some(_) => {}
                None if self.store.inner.has(hash)? => {}
                None => {
                    let noted = Noted { block, owner };
                    if let Some(ledger) = &notes.ledger {
                        ledger.note(hash, noted)?;
                    }
                    notes.noted.insert(hash, noted);
                }
            }
        }
        // Pinned, so no collection removes it meanwhile.
        self.store.inner.put(sealed)?;
        self.sealed
            .write()
            .expect("never poisoned")
            .insert(hash, key);
        Ok(hash)
    }

    fn write_part(
        &self,
        key: Uuid,
        sealing: &(ContentKey, Uuid),
        part: &Part<'_>,
    ) -> Result<Option<PartRef>, StoreError> {
        match part {
            Part::Bytes(bytes) => {
                let mut writer = self.writer(key, *sealing, None);
                writer.write(bytes)?;
                Ok(writer.finish()?.part())
            }
            Part::Reused(part) => match self.sealed_under(part.hash())? == key {
                true => Ok(Some(*part)),
                false => self.reseal(key, sealing, *part).map(Some),
            },
            Part::Node {
                parts: children,
                references,
            } => {
                let mut parts = Vec::with_capacity(children.len());
                for child in children {
                    parts.extend(self.write_part(key, sealing, child)?);
                }
                if parts.is_empty() && references.is_empty() {
                    return Ok(None);
                }
                let node = Node {
                    parts,
                    references: references.clone(),
                };
                Ok(Some(PartRef::Node(self.put_node(key, sealing, node, None)?)))
            }
        }
    }

    /// Groups one part's chunks under nodes chosen by the chunks' hashes, as a prolly tree
    /// does, so a changed chunk rewrites only the nodes above it.
    fn group(
        &self,
        key: Uuid,
        sealing: &(ContentKey, Uuid),
        mut parts: Vec<PartRef>,
        mut pins: Option<&mut Pins>,
    ) -> Result<Option<PartRef>, StoreError> {
        loop {
            if parts.len() <= 1 {
                return Ok(parts.pop());
            }
            let mut grouped = Vec::new();
            let mut group = Vec::new();
            for part in parts {
                group.push(part);
                let boundary =
                    u16::from_le_bytes([part.hash().as_bytes()[0], part.hash().as_bytes()[1]])
                        % GROUP_AVERAGE
                        == 0;
                if (boundary && group.len() >= GROUP_MINIMUM) || group.len() >= GROUP_MAXIMUM {
                    grouped.push(std::mem::take(&mut group));
                }
            }
            if !group.is_empty() {
                grouped.push(group);
            }
            if grouped.len() == 1 {
                let parts = grouped.pop().expect("one group");
                let node = Node {
                    parts,
                    references: Vec::new(),
                };
                return Ok(Some(PartRef::Chunks(self.put_node(
                    key,
                    sealing,
                    node,
                    pins.as_deref_mut(),
                )?)));
            }
            parts = grouped
                .into_iter()
                .map(|parts| {
                    let node = Node {
                        parts,
                        references: Vec::new(),
                    };
                    self.put_node(key, sealing, node, pins.as_deref_mut())
                        .map(PartRef::Chunks)
                })
                .collect::<Result<_, _>>()?;
        }
    }

    /// The key an object was sealed under. A subtree is sealed under one key, since a write
    /// reseals every reused part sealed under another, so its top object's key is all of
    /// its keys.
    pub fn sealed_under(&self, hash: Hash) -> Result<Uuid, StoreError> {
        if let Some(key) = self.sealed.read().expect("never poisoned").get(&hash) {
            return Ok(*key);
        }
        let sealed = self
            .store
            .get(hash)?
            .ok_or(StoreError::MissingObject(hash))?;
        let key = sealed_key(&sealed)?;
        self.sealed
            .write()
            .expect("never poisoned")
            .insert(hash, key);
        Ok(key)
    }

    fn put_sealed(&self, key: Uuid, sealed: &[u8]) -> Result<Hash, StoreError> {
        let hash = self.store.put(sealed)?;
        self.sealed
            .write()
            .expect("never poisoned")
            .insert(hash, key);
        Ok(hash)
    }

    /// Leaves `objects` out of what a collection may remove: they were looked for by something
    /// other than the part writer that noted them, which may name them now (`Claim::Looked`).
    fn claim(&self, objects: &[Hash]) -> Result<(), StoreError> {
        if COLLECTING.get() {
            return Ok(());
        }
        self.store
            .unnamed
            .lock()
            .claim(objects, self.store.owner, Claim::Looked)
    }

    /// How many objects its part writers sealed that a collection has not yet found named or
    /// removed.
    pub fn unnamed_count(&self) -> usize {
        let owner = self.store.owner;
        let notes = self.store.unnamed.lock();
        notes
            .noted
            .values()
            .filter(|noted| noted.owner == owner)
            .count()
    }

    /// Removes the objects this vault's part writers sealed that nothing names: `named` is
    /// given those unpinned, by the block whose key sealed them, and answers the ones something
    /// on this device names (a held commit, an outbox entry); it takes out of the candidates
    /// the blocks whose objects it cannot tell about yet (edits kept that may name them),
    /// which stay noted for a later collection. The rest are removed, and those named are no
    /// longer noted, being held as any commit's are. It all runs under the lock part writers
    /// pin under, shared by every vault over the store, and removes nothing while an access
    /// through any of them is under way (`VaultStore`); `named` reads through this vault without
    /// locking or claiming, and must not seal or put. Answers how many objects it removed.
    pub fn collect(
        &self,
        named: impl FnOnce(&mut HashMap<Uuid, HashSet<Hash>>) -> Result<HashSet<Hash>, StoreError>,
    ) -> Result<usize, StoreError> {
        let owner = self.store.owner;
        let mut notes = self.store.unnamed.lock();
        if notes.accessing > 0 {
            return Ok(0);
        }
        let mut candidates: HashMap<Uuid, HashSet<Hash>> = HashMap::new();
        for (object, noted) in &notes.noted {
            if noted.owner == owner && !notes.pinned.contains_key(object) {
                candidates.entry(noted.block).or_default().insert(*object);
            }
        }
        if candidates.is_empty() {
            return Ok(0);
        }
        let named = {
            struct Collecting;
            impl Drop for Collecting {
                fn drop(&mut self) {
                    COLLECTING.set(false);
                }
            }
            COLLECTING.set(true);
            let _collecting = Collecting;
            named(&mut candidates)?
        };
        let mut forgotten = Vec::new();
        let mut removed = 0;
        for object in candidates.into_values().flatten() {
            if !named.contains(&object) {
                self.store.inner.remove(object)?;
                self.sealed.write().expect("never poisoned").remove(&object);
                removed += 1;
            }
            forgotten.push(object);
        }
        notes.forget(&forgotten)?;
        Ok(removed)
    }

    /// Adds to `marked` every object of the manifest tree this store holds, going into no node
    /// already marked or missing: what a collection (`collect`) keeps. Answers those it added.
    pub fn mark(
        &self,
        manifest: &Manifest,
        marked: &mut HashSet<Hash>,
    ) -> Result<Vec<Hash>, StoreError> {
        let mut added = Vec::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            let hash = part.hash();
            if !self.store.has(hash)? || !marked.insert(hash) {
                return Ok(false);
            }
            added.push(hash);
            Ok(true)
        })?;
        Ok(added)
    }

    /// The reused parts of `part` sealed under a key other than `key`, which a write under
    /// `key` reseals and so needs every object of: a part whose top object is missing is
    /// listed too, since its key is unknown.
    pub fn stale_parts(&self, key: Uuid, part: &Part<'_>) -> Result<Vec<PartRef>, StoreError> {
        let mut stale = Vec::new();
        let mut parts = vec![part];
        while let Some(part) = parts.pop() {
            match part {
                Part::Bytes(_) => {}
                Part::Reused(reused) => {
                    let current = match self.store.has(reused.hash())? {
                        true => self.sealed_under(reused.hash())? == key,
                        false => false,
                    };
                    if !current {
                        stale.push(*reused);
                    }
                }
                Part::Node {
                    parts: children, ..
                } => parts.extend(children),
            }
        }
        Ok(stale)
    }

    /// Seals every object of a saved part again under `key`: after a rotation, a part kept
    /// from the head must not stay readable with the old key alone.
    fn reseal(
        &self,
        key: Uuid,
        sealing: &(ContentKey, Uuid),
        part: PartRef,
    ) -> Result<PartRef, StoreError> {
        match part {
            PartRef::Chunk(chunk) => {
                let sealed = self
                    .store
                    .get(chunk.hash)?
                    .ok_or(StoreError::MissingObject(chunk.hash))?;
                let plain = self.open(&sealed)?;
                if plain.len() != chunk.length as usize {
                    return Err(StoreError::Corrupt);
                }
                let hash = self.put_sealed(key, &sealing.0.seal(key, sealing.1, &plain, true))?;
                Ok(PartRef::Chunk(ChunkRef {
                    hash,
                    length: chunk.length,
                }))
            }
            PartRef::Chunks(held) | PartRef::Node(held) => {
                let node = self.node(held)?;
                let mut parts = Vec::with_capacity(node.parts.len());
                for child in node.parts {
                    parts.push(self.reseal(key, sealing, child)?);
                }
                let resealed = self.put_node(
                    key,
                    sealing,
                    Node {
                        parts,
                        references: node.references,
                    },
                    None,
                )?;
                Ok(match part {
                    PartRef::Chunks(_) => PartRef::Chunks(resealed),
                    _ => PartRef::Node(resealed),
                })
            }
        }
    }

    fn put_node(
        &self,
        key: Uuid,
        sealing: &(ContentKey, Uuid),
        node: Node,
        pins: Option<&mut Pins>,
    ) -> Result<NodeRef, StoreError> {
        let length = node.parts.iter().map(|part| part.length()).sum();
        let sealed = sealing.0.seal(key, sealing.1, &encode_value(&node)?, true);
        let hash = self.put_written(key, sealing.1, &sealed, pins)?;
        Ok(NodeRef { hash, length })
    }

    fn node(&self, node: NodeRef) -> Result<Node, StoreError> {
        let node_value: Node = self.get_value(node.hash)?;
        if node_value
            .parts
            .iter()
            .map(|part| part.length())
            .sum::<u64>()
            != node.length
        {
            return Err(StoreError::Corrupt);
        }
        Ok(node_value)
    }

    /// Visits every part under `part` that overlaps `range`, a node before its parts, with
    /// where it starts in the stored form. `visit` answers whether to go into a node.
    fn walk(
        &self,
        part: PartRef,
        start: u64,
        range: &Range<u64>,
        visit: &mut impl FnMut(PartRef, u64) -> Result<bool, StoreError>,
    ) -> Result<(), StoreError> {
        let end = start + part.length();
        let overlaps = match part.length() {
            0 => range.contains(&start),
            _ => end > range.start && start < range.end,
        };
        if !overlaps {
            return Ok(());
        }
        if !visit(part, start)? {
            return Ok(());
        }
        if let Some(node) = part.node() {
            let mut at = start;
            for child in self.node(node)?.parts {
                self.walk(child, at, range, visit)?;
                at += child.length();
            }
        }
        Ok(())
    }

    fn walk_manifest(
        &self,
        manifest: &Manifest,
        range: Range<u64>,
        mut visit: impl FnMut(PartRef, u64) -> Result<bool, StoreError>,
    ) -> Result<(), StoreError> {
        match manifest.root {
            Some(root) => self.walk(root, 0, &range, &mut visit),
            None => Ok(()),
        }
    }

    pub fn read(&self, manifest: &Manifest) -> Result<Vec<u8>, StoreError> {
        let data = self.read_range(manifest, 0, manifest.length())?;
        if data.len() as u64 != manifest.length() {
            return Err(StoreError::Corrupt);
        }
        Ok(data)
    }

    /// The stored form and the parts it was saved in, in the shape the content wrote them.
    pub fn load(&self, manifest: &Manifest) -> Result<Stored, StoreError> {
        let bytes = self.read(manifest)?;
        let root = match manifest.root {
            Some(root) => Some(self.loaded(root, 0)?),
            None => None,
        };
        Ok(Stored { bytes, root })
    }

    /// The parts of the stored form, without its bytes. Fails on a node this store lacks.
    pub fn outline(&self, manifest: &Manifest) -> Result<Option<LoadedPart>, StoreError> {
        manifest.root.map(|root| self.loaded(root, 0)).transpose()
    }

    /// Like `load`, but `Stored::bytes` holds only the root's first part (all of a stored
    /// form that has one part), for content that loads the rest on demand
    /// (`be_block_content::Loads::FirstPart`).
    pub fn load_first(&self, manifest: &Manifest) -> Result<Stored, StoreError> {
        let root = self.outline(manifest)?;
        let end = root.as_ref().map_or(0, LoadedPart::first_end);
        let bytes = self.read_range(manifest, 0, end)?;
        if bytes.len() as u64 != end {
            return Err(StoreError::Corrupt);
        }
        Ok(Stored { bytes, root })
    }

    /// `length` bytes of a saved part from `offset`, as `read_range` reads a manifest's.
    pub fn read_part(
        &self,
        part: PartRef,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, StoreError> {
        let manifest = Manifest {
            content_type: Uuid::nil(),
            root: Some(part),
        };
        self.read_range(&manifest, offset, length)
    }

    fn loaded(&self, reference: PartRef, start: u64) -> Result<LoadedPart, StoreError> {
        let mut children = Vec::new();
        let mut references = Vec::new();
        if let PartRef::Node(node) = reference {
            let node = self.node(node)?;
            let mut at = start;
            for child in node.parts {
                children.push(self.loaded(child, at)?);
                at += child.length();
            }
            references = node.references;
        }
        Ok(LoadedPart {
            reference,
            start,
            children,
            references,
        })
    }

    /// The references of every node of the manifest tree (`Part::Node`), in tree order, each
    /// once. Fails on a node this store does not hold.
    pub fn references(&self, manifest: &Manifest) -> Result<Vec<Uuid>, StoreError> {
        let mut references = Vec::new();
        let mut seen = HashSet::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            let PartRef::Node(node) = part else {
                return Ok(false);
            };
            for reference in self.node(node)?.references {
                if seen.insert(reference) {
                    references.push(reference);
                }
            }
            Ok(true)
        })?;
        Ok(references)
    }

    pub fn read_range(
        &self,
        manifest: &Manifest,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, StoreError> {
        let end = offset.saturating_add(length).min(manifest.length());
        if offset >= end {
            return Ok(Vec::new());
        }
        let mut data = Vec::with_capacity(usize::try_from(end - offset).unwrap_or(0));
        self.walk_manifest(manifest, offset..end, |part, position| {
            let PartRef::Chunk(chunk) = part else {
                return Ok(true);
            };
            let sealed = self
                .store
                .get(chunk.hash)?
                .ok_or(StoreError::MissingObject(chunk.hash))?;
            let plain = self.open(&sealed)?;
            if plain.len() != chunk.length as usize {
                return Err(StoreError::Corrupt);
            }
            let start = usize::try_from(offset.saturating_sub(position)).unwrap_or(0);
            let stop = usize::try_from((end - position).min(u64::from(chunk.length))).unwrap_or(0);
            data.extend_from_slice(&plain[start..stop]);
            Ok(false)
        })?;
        Ok(data)
    }

    /// Every object of the manifest tree: its nodes and chunks. Fails on a node this store
    /// does not hold.
    pub fn objects(&self, manifest: &Manifest) -> Result<Vec<Hash>, StoreError> {
        self.objects_besides(manifest, &HashSet::new())
    }

    /// The manifest's objects that are not under a node or chunk in `known`, such as the
    /// objects of the commit it was loaded from.
    pub fn objects_besides(
        &self,
        manifest: &Manifest,
        known: &HashSet<Hash>,
    ) -> Result<Vec<Hash>, StoreError> {
        let mut objects = Vec::new();
        let mut seen = HashSet::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            let hash = part.hash();
            if known.contains(&hash) || !seen.insert(hash) {
                return Ok(false);
            }
            objects.push(hash);
            Ok(true)
        })?;
        Ok(objects)
    }

    /// The objects under `offset..offset + length` that this store lacks and can name: a
    /// missing node hides what is under it, so fetch these and ask again until none are left.
    pub fn missing_objects(
        &self,
        manifest: &Manifest,
        offset: u64,
        length: u64,
    ) -> Result<Vec<Hash>, StoreError> {
        let mut missing = Vec::new();
        let mut seen = HashSet::new();
        let end = offset.saturating_add(length);
        self.walk_manifest(manifest, offset..end, |part, _| {
            let hash = part.hash();
            if !seen.insert(hash) {
                return Ok(false);
            }
            if self.store.has(hash)? {
                return Ok(true);
            }
            missing.push(hash);
            Ok(false)
        })?;
        self.claim(&seen.into_iter().collect::<Vec<_>>())?;
        Ok(missing)
    }

    /// Whether this store holds every object of the manifest, stopping at the first it lacks.
    pub fn holds_all(&self, manifest: &Manifest) -> Result<bool, StoreError> {
        let mut whole = true;
        let mut seen = HashSet::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            if !whole || !seen.insert(part.hash()) {
                return Ok(false);
            }
            whole = self.store.has(part.hash())?;
            Ok(whole)
        })?;
        self.claim(&seen.into_iter().collect::<Vec<_>>())?;
        Ok(whole)
    }

    /// The manifest's nodes this store lacks and can name, leaving chunks out.
    pub fn missing_nodes(&self, manifest: &Manifest) -> Result<Vec<Hash>, StoreError> {
        let mut missing = Vec::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            if part.node().is_none() {
                return Ok(false);
            }
            if self.store.has(part.hash())? {
                return Ok(true);
            }
            missing.push(part.hash());
            Ok(false)
        })?;
        Ok(missing)
    }

    /// The nodes of the parts the content wrote (`PartRef::Node`) this store lacks and can
    /// name, leaving out the vault's split parts (`PartRef::Chunks`) and chunks: what `outline`
    /// reads, so it costs the content's shape, not the size of its bytes.
    pub fn missing_outline(&self, manifest: &Manifest) -> Result<Vec<Hash>, StoreError> {
        let mut missing = Vec::new();
        self.walk_manifest(manifest, 0..u64::MAX, |part, _| {
            let PartRef::Node(node) = part else {
                return Ok(false);
            };
            if self.store.has(node.hash)? {
                return Ok(true);
            }
            missing.push(node.hash);
            Ok(false)
        })?;
        Ok(missing)
    }

    /// Stores a value as a CBOR map by field name (`encode_value`), so a later build can add a
    /// field with `#[serde(default)]`. Values stored this way (commits, version-control trees)
    /// are encrypted, so only a client could rewrite one, and that would change its hash and
    /// every id naming it: read them as they were written, forever.
    pub fn put_value<T: Serialize>(&self, key: Uuid, value: &T) -> Result<Hash, StoreError> {
        self.store
            .put(&self.keys.seal_content(key, &encode_value(value)?)?)
    }

    pub fn get_value<T: DeserializeOwned>(&self, hash: Hash) -> Result<T, StoreError> {
        let sealed = self
            .store
            .get(hash)?
            .ok_or(StoreError::MissingObject(hash))?;
        decode_value(&self.open(&sealed)?)
    }

    pub fn has_value(&self, hash: Hash) -> Result<bool, StoreError> {
        self.store.has(hash)
    }
}
