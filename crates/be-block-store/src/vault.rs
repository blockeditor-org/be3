use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use rand::TryRngCore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use crate::{
    StoreError,
    chunker::{ChunkerConfig, split},
    hash::Hash,
    manifest::{
        ChunkRef, LoadedPart, Manifest, Node, NodeRef, Part, PartRef, Stored, decode_value,
        encode_value,
    },
    store::ObjectStore,
};

pub const NONCE_LEN: usize = 12;
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

    pub fn seal(&self, id: Uuid, plain: &[u8]) -> Vec<u8> {
        let nonce_source = Hash::of_parts(&[self.as_bytes(), plain]);
        let nonce_bytes = &nonce_source.as_bytes()[..NONCE_LEN];
        let cipher = ChaCha20Poly1305::new(Key::from_slice(self.as_bytes()));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(nonce_bytes), plain)
            .expect("chacha20poly1305 never fails on a valid key and nonce");
        let mut sealed = Vec::with_capacity(HEADER_LEN + NONCE_LEN + ciphertext.len());
        sealed.extend_from_slice(TAG);
        sealed.extend_from_slice(id.as_bytes());
        sealed.extend_from_slice(nonce_bytes);
        sealed.extend_from_slice(&ciphertext);
        sealed
    }

    fn open_body(&self, body: &[u8]) -> Result<Vec<u8>, StoreError> {
        let (nonce_bytes, ciphertext) = body
            .split_at_checked(NONCE_LEN)
            .ok_or(StoreError::Corrupt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(self.as_bytes()));
        cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| StoreError::Corrupt)
    }
}

impl std::fmt::Debug for ContentKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContentKey(..)")
    }
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
pub struct Keyring {
    keys: Arc<RwLock<HashMap<Uuid, ContentKey>>>,
}

impl Keyring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, id: Uuid, key: ContentKey) {
        self.keys
            .write()
            .expect("the keyring is never poisoned")
            .insert(id, key);
    }

    pub fn get(&self, id: Uuid) -> Option<ContentKey> {
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

    pub fn seal(&self, key: Uuid, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(self
            .get(key)
            .ok_or(StoreError::MissingKey(key))?
            .seal(key, plain))
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, StoreError> {
        let id = sealed_key(sealed)?;
        let key = self.get(id).ok_or(StoreError::MissingKey(id))?;
        key.open_body(&sealed[HEADER_LEN..])
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
    store: S,
    keys: Keyring,
    chunker: ChunkerConfig,
    sealed: RwLock<HashMap<Hash, Uuid>>,
}

impl<S: ObjectStore> Vault<S> {
    pub fn new(store: S, keys: Keyring) -> Self {
        Self {
            store,
            keys,
            chunker: ChunkerConfig::DEFAULT,
            sealed: RwLock::default(),
        }
    }

    pub fn with_chunker(mut self, chunker: ChunkerConfig) -> Self {
        self.chunker = chunker;
        self
    }

    pub fn store(&self) -> &S {
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
        let sealing = self.keys.get(key).ok_or(StoreError::MissingKey(key))?;
        let root = self.write_part(key, &sealing, &parts.into())?;
        Ok(Manifest { content_type, root })
    }

    fn write_part(
        &self,
        key: Uuid,
        sealing: &ContentKey,
        part: &Part<'_>,
    ) -> Result<Option<PartRef>, StoreError> {
        match part {
            Part::Bytes(bytes) => {
                let mut chunks = Vec::new();
                for range in split(bytes, self.chunker) {
                    let length =
                        u32::try_from(range.len()).map_err(|_| StoreError::ChunkTooLarge)?;
                    let hash = self.put_sealed(key, &sealing.seal(key, &bytes[range]))?;
                    chunks.push(PartRef::Chunk(ChunkRef { hash, length }));
                }
                self.group(key, sealing, chunks)
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
                Ok(Some(PartRef::Node(self.put_node(key, sealing, node)?)))
            }
        }
    }

    /// Groups one part's chunks under nodes chosen by the chunks' hashes, as a prolly tree
    /// does, so a changed chunk rewrites only the nodes above it.
    fn group(
        &self,
        key: Uuid,
        sealing: &ContentKey,
        mut parts: Vec<PartRef>,
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
                return Ok(Some(PartRef::Chunks(self.put_node(key, sealing, node)?)));
            }
            parts = grouped
                .into_iter()
                .map(|parts| {
                    let node = Node {
                        parts,
                        references: Vec::new(),
                    };
                    self.put_node(key, sealing, node).map(PartRef::Chunks)
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
        sealing: &ContentKey,
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
                let hash = self.put_sealed(key, &sealing.seal(key, &plain))?;
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
                )?;
                Ok(match part {
                    PartRef::Chunks(_) => PartRef::Chunks(resealed),
                    _ => PartRef::Node(resealed),
                })
            }
        }
    }

    fn put_node(&self, key: Uuid, sealing: &ContentKey, node: Node) -> Result<NodeRef, StoreError> {
        let length = node.parts.iter().map(|part| part.length()).sum();
        let hash = self.put_sealed(key, &sealing.seal(key, &encode_value(&node)?))?;
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
    /// (`be_block::Loads::FirstPart`).
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
        Ok(missing)
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

    /// Stores a value as a CBOR map by field name (`encode_value`), so a later build can add a
    /// field with `#[serde(default)]`. Values stored this way (commits, version-control trees)
    /// are encrypted, so only a client could rewrite one, and that would change its hash and
    /// every id naming it: read them as they were written, forever.
    pub fn put_value<T: Serialize>(&self, key: Uuid, value: &T) -> Result<Hash, StoreError> {
        self.store.put(&self.seal(key, &encode_value(value)?)?)
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
