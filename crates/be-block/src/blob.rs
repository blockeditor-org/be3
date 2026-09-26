use std::fmt::Debug;

use be_commit::MergeResult;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, DerivedMetadata, LiveEdit, Merge,
    streamed::{Streamed, decode_streamed, encode_streamed},
};

pub trait BlobKind: Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    type Header: Clone
        + Debug
        + Default
        + PartialEq
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + 'static;

    fn name(header: &Self::Header) -> &str;

    fn derived_metadata(header: &Self::Header) -> DerivedMetadata {
        let _ = header;
        DerivedMetadata::default()
    }
}

pub struct Blob<K: BlobKind> {
    header: K::Header,
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BlobOp<H> {
    SetHeader(H),
}

impl<K: BlobKind> Blob<K> {
    pub fn new(header: K::Header, data: Vec<u8>) -> Self {
        Self { header, data }
    }

    pub const fn header(&self) -> &K::Header {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn set_header(&mut self, header: K::Header) {
        self.header = header;
    }
}

impl<K: BlobKind> Clone for Blob<K> {
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            data: self.data.clone(),
        }
    }
}

impl<K: BlobKind> Default for Blob<K> {
    fn default() -> Self {
        Self {
            header: K::Header::default(),
            data: Vec::new(),
        }
    }
}

impl<K: BlobKind> PartialEq for Blob<K> {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.data == other.data
    }
}

impl<K: BlobKind> Eq for Blob<K> where K::Header: Eq {}

impl<K: BlobKind> Debug for Blob<K> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Blob")
            .field("header", &self.header)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl<K: BlobKind> BlockContent for Blob<K> {
    const CONTENT_TYPE: Uuid = K::CONTENT_TYPE;

    fn encode(&self) -> Vec<u8> {
        encode_streamed(&self.header, &self.data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let (header, data) = decode_streamed(bytes)?;
        Ok(Self { header, data })
    }

    fn name(&self) -> Option<String> {
        let name = K::name(&self.header).trim();
        (!name.is_empty()).then(|| name.to_owned())
    }

    fn derived_metadata(&self) -> DerivedMetadata {
        K::derived_metadata(&self.header)
    }
}

impl<K: BlobKind> Streamed for Blob<K> {
    type Header = K::Header;

    fn header(&self) -> Self::Header {
        self.header.clone()
    }

    fn payload(&self) -> &[u8] {
        &self.data
    }

    fn from_parts(header: Self::Header, payload: Vec<u8>) -> Self {
        Self {
            header,
            data: payload,
        }
    }
}

impl<K: BlobKind> LiveEdit for Blob<K> {
    type Op = BlobOp<K::Header>;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            BlobOp::SetHeader(header) => self.header.clone_from(header),
        }
    }
}

impl<K: BlobKind> Merge for Blob<K> {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        if ours == theirs || theirs == base {
            return MergeResult::Clean(ours.clone());
        }
        if ours == base {
            return MergeResult::Clean(theirs.clone());
        }
        MergeResult::Conflicted {
            value: ours.clone(),
            conflicts: 1,
        }
    }
}
