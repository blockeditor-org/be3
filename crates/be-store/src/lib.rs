use std::{error::Error, fmt, io};

pub mod chunker;
pub mod hash;
pub mod store;
pub mod vault;

pub use chunker::{ChunkerConfig, split};
pub use hash::Hash;
pub use store::{FileStore, MemoryStore, ObjectStore};
pub use vault::{ChunkRef, ContentKey, Manifest, Vault};

#[derive(Debug)]
pub enum StoreError {
    MissingObject(Hash),
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
