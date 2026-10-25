use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{StoreError, hash::Hash};

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

pub fn encode_value<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let mut encoded = Vec::new();
    ciborium::into_writer(value, &mut encoded).map_err(|_| StoreError::Encoding)?;
    Ok(encoded)
}

pub fn decode_value<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    ciborium::from_reader(bytes).map_err(|_| StoreError::Encoding)
}
