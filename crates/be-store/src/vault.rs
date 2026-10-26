use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use rand::TryRngCore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use crate::{
    StoreError,
    chunker::{ChunkerConfig, split},
    hash::Hash,
    manifest::{ChunkRef, Manifest, decode_value, encode_value},
    store::ObjectStore,
};

pub const NONCE_LEN: usize = 12;
pub const TAG: &[u8; 4] = b"be3K";
const HEADER_LEN: usize = TAG.len() + 16;

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

pub struct Vault<S> {
    store: S,
    keys: Keyring,
    chunker: ChunkerConfig,
}

impl<S: ObjectStore> Vault<S> {
    pub fn new(store: S, keys: Keyring) -> Self {
        Self {
            store,
            keys,
            chunker: ChunkerConfig::DEFAULT,
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

    pub fn write(
        &self,
        key: Uuid,
        content_type: Uuid,
        data: &[u8],
    ) -> Result<Manifest, StoreError> {
        let sealing = self.keys.get(key).ok_or(StoreError::MissingKey(key))?;
        let mut chunks = Vec::new();
        for range in split(data, self.chunker) {
            let length = u32::try_from(range.len()).map_err(|_| StoreError::ChunkTooLarge)?;
            let hash = self.store.put(&sealing.seal(key, &data[range]))?;
            chunks.push(ChunkRef { hash, length });
        }
        Ok(Manifest {
            content_type,
            length: data.len() as u64,
            chunks,
        })
    }

    pub fn read(&self, manifest: &Manifest) -> Result<Vec<u8>, StoreError> {
        let mut data = Vec::with_capacity(usize::try_from(manifest.length).unwrap_or(0));
        for chunk in &manifest.chunks {
            let sealed = self
                .store
                .get(chunk.hash)?
                .ok_or(StoreError::MissingObject(chunk.hash))?;
            data.extend_from_slice(&self.open(&sealed)?);
        }
        if data.len() as u64 != manifest.length {
            return Err(StoreError::Corrupt);
        }
        Ok(data)
    }

    pub fn read_range(
        &self,
        manifest: &Manifest,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, StoreError> {
        let end = offset.saturating_add(length).min(manifest.length);
        if offset >= end {
            return Ok(Vec::new());
        }
        let mut data = Vec::with_capacity(usize::try_from(end - offset).unwrap_or(0));
        let mut position = 0u64;
        for chunk in &manifest.chunks {
            let chunk_end = position + u64::from(chunk.length);
            if chunk_end <= offset {
                position = chunk_end;
                continue;
            }
            if position >= end {
                break;
            }
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
            position = chunk_end;
        }
        Ok(data)
    }

    pub fn chunks_in_range(&self, manifest: &Manifest, offset: u64, length: u64) -> Vec<ChunkRef> {
        let end = offset.saturating_add(length).min(manifest.length);
        let mut position = 0u64;
        let mut overlapping = Vec::new();
        for chunk in &manifest.chunks {
            let chunk_end = position + u64::from(chunk.length);
            if chunk_end > offset && position < end {
                overlapping.push(*chunk);
            }
            position = chunk_end;
        }
        overlapping
    }

    pub fn missing_chunks(&self, manifest: &Manifest) -> Result<Vec<Hash>, StoreError> {
        let mut missing = Vec::new();
        for chunk in &manifest.chunks {
            if !self.store.has(chunk.hash)? {
                missing.push(chunk.hash);
            }
        }
        Ok(missing)
    }

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
