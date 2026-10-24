use std::{error::Error, fmt, io};

pub mod bytes;
pub mod chunker;
pub mod hash;
pub mod manifest;
pub mod store;
#[cfg(feature = "vault")]
pub mod vault;

pub use chunker::{ChunkerConfig, split};
pub use hash::Hash;
pub use manifest::{ChunkRef, Manifest, decode_value, encode_value};
pub use store::{FileStore, MemoryStore, ObjectStore};
#[cfg(feature = "vault")]
pub use vault::{ContentKey, Keyring, Vault, sealed_key};

#[derive(Debug)]
pub enum StoreError {
    MissingObject(Hash),
    MissingKey(uuid::Uuid),
    Corrupt,
    ChunkTooLarge,
    Encoding,
    InvalidPath,
    Io(io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingObject(hash) => write!(formatter, "object {hash} is not in the store"),
            Self::MissingKey(key) => write!(formatter, "key {key} is not on this keyring"),
            Self::Corrupt => formatter.write_str("stored bytes did not decrypt to valid content"),
            Self::ChunkTooLarge => formatter.write_str("a chunk exceeded the addressable length"),
            Self::Encoding => formatter.write_str("an object failed to encode or decode"),
            Self::InvalidPath => formatter.write_str("the store path has no parent directory"),
            Self::Io(error) => write!(formatter, "store io failed: {error}"),
        }
    }
}

impl Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests;
