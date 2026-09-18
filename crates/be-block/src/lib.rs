use std::{error::Error, fmt};

use be_commit::MergeResult;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod image;
pub mod streamed;
pub mod text;

pub use image::{ImageContent, ImageHeader};
pub use streamed::{
    HEADER_PREFIX_BYTES, Streamed, decode_streamed, encode_streamed, payload_start,
};
pub use text::{TextContent, TextHeader, TextLanguage, TextOp};

#[derive(Debug, Eq, PartialEq)]
pub enum ContentError {
    Malformed(&'static str),
    Truncated,
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(formatter, "stored content is malformed: {what}"),
            Self::Truncated => formatter.write_str("stored content ended early"),
        }
    }
}

impl Error for ContentError {}

pub trait BlockContent: Sized + Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    fn encode(&self) -> Vec<u8>;

    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn name(&self) -> Option<String> {
        None
    }
}

pub trait LiveEdit: BlockContent {
    type Op: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn apply(&mut self, operation: &Self::Op);

    fn rebase(operation: Self::Op, onto: &[Self::Op]) -> Option<Self::Op> {
        let _ = onto;
        Some(operation)
    }

    fn encode_operation(operation: &Self::Op) -> Vec<u8> {
        postcard::to_stdvec(operation).unwrap_or_default()
    }

    fn decode_operation(bytes: &[u8]) -> Result<Self::Op, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("operation"))
    }
}

pub trait Merge: BlockContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self>;
}

#[cfg(test)]
mod tests;
