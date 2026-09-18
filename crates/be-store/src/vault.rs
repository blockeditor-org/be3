use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use rand::RngCore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    StoreError,
    chunker::{ChunkerConfig, split},
    hash::Hash,
    store::ObjectStore,
};

pub const NONCE_LEN: usize = 12;

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentKey([u8; 32]);

impl ContentKey {
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for ContentKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContentKey(..)")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChunkRef {
    pub hash: Hash,
    pub length: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub content_type: Uuid,
    pub length: u64,
    pub chunks: Vec<ChunkRef>,
}

impl Manifest {
    pub fn chunk_hashes(&self) -> Vec<Hash> {
        self.chunks.iter().map(|chunk| chunk.hash).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub struct Vault<S> {
    store: S,
    key: ContentKey,
    chunker: ChunkerConfig,
}

impl<S: ObjectStore> Vault<S> {
    pub fn new(store: S, key: ContentKey) -> Self {
        Self {
            store,
            key,
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

    pub fn key(&self) -> ContentKey {
        self.key
    }

    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let nonce_source = Hash::of_parts(&[self.key.as_bytes(), plain]);
        let nonce_bytes = &nonce_source.as_bytes()[..NONCE_LEN];
        let cipher = ChaCha20Poly1305::new(Key::from_slice(self.key.as_bytes()));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(nonce_bytes), plain)
            .expect("chacha20poly1305 never fails on a valid key and nonce");
        let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        sealed.extend_from_slice(nonce_bytes);
        sealed.extend_from_slice(&ciphertext);
        sealed
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, StoreError> {
        let (nonce_bytes, ciphertext) = sealed
            .split_at_checked(NONCE_LEN)
            .ok_or(StoreError::Corrupt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(self.key.as_bytes()));
        cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| StoreError::Corrupt)
    }

    pub fn write(&self, content_type: Uuid, data: &[u8]) -> Result<Manifest, StoreError> {
        let mut chunks = Vec::new();
        for range in split(data, self.chunker) {
            let length = u32::try_from(range.len()).map_err(|_| StoreError::ChunkTooLarge)?;
            let hash = self.store.put(&self.seal(&data[range]))?;
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

    pub fn put_value<T: Serialize>(&self, value: &T) -> Result<Hash, StoreError> {
        let encoded = postcard::to_stdvec(value).map_err(|_| StoreError::Encoding)?;
        self.store.put(&self.seal(&encoded))
    }

    pub fn get_value<T: DeserializeOwned>(&self, hash: Hash) -> Result<T, StoreError> {
        let sealed = self
            .store
            .get(hash)?
            .ok_or(StoreError::MissingObject(hash))?;
        let plain = self.open(&sealed)?;
        postcard::from_bytes(&plain).map_err(|_| StoreError::Encoding)
    }

    pub fn has_value(&self, hash: Hash) -> Result<bool, StoreError> {
        self.store.has(hash)
    }
}
