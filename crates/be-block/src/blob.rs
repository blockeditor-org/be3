use std::fmt::Debug;

use be_block_commit::MergeResult;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    BlockContent, BlockMetadata, ContentError, LiveEdit, Loads, Merge, Part, PartRef, Stored,
    streamed::{Streamed, decode_streamed, encode_streamed},
};

pub trait BlobKind: Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    const FROZEN: bool = false;

    const SCHEMA: u32 = 0;

    /// `Loads::FirstPart` leaves the payload in the store for editors to read by range
    /// (`Blob::is_loaded`); a kind switches once its editors read it that way.
    const LOADS: Loads = Loads::Whole;

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

    fn describe(header: &Self::Header, metadata: &mut BlockMetadata) {
        let _ = (header, metadata);
    }
}

/// Saved as two parts, the encoded header and the payload, so a header change seals and
/// uploads only the header. A blob loaded with `from_stored` hands its payload back as
/// `Part::Reused`.
pub struct Blob<K: BlobKind> {
    header: K::Header,
    data: Vec<u8>,
    saved: Option<PartRef>,
    unloaded: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BlobOp<H> {
    SetHeader(H),
}

impl<K: BlobKind> Blob<K> {
    pub fn new(header: K::Header, data: Vec<u8>) -> Self {
        Self {
            header,
            data,
            saved: None,
            unloaded: false,
        }
    }

    pub const fn header(&self) -> &K::Header {
        &self.header
    }

    /// The payload, empty while it is not loaded (`is_loaded`).
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// False for a blob of a kind that loads only its header (`BlobKind::LOADS`), whose
    /// payload stays in the store.
    pub const fn is_loaded(&self) -> bool {
        !self.unloaded
    }

    pub fn payload_length(&self) -> u64 {
        match self.unloaded {
            true => self.saved.map_or(0, PartRef::length),
            false => self.data.len() as u64,
        }
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
            saved: self.saved,
            unloaded: self.unloaded,
        }
    }
}

impl<K: BlobKind> Default for Blob<K> {
    fn default() -> Self {
        Self {
            header: K::Header::default(),
            data: Vec::new(),
            saved: None,
            unloaded: false,
        }
    }
}

impl<K: BlobKind> PartialEq for Blob<K> {
    fn eq(&self, other: &Self) -> bool {
        let payload = match (self.unloaded, other.unloaded) {
            (false, false) => self.data == other.data,
            (true, true) => self.saved == other.saved,
            _ => false,
        };
        self.header == other.header && payload
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

    const FROZEN: bool = K::FROZEN;

    const LOADS: Loads = K::LOADS;

    fn schema() -> String {
        format!("blob {}", K::SCHEMA)
    }

    fn encode(&self) -> Vec<u8> {
        encode_streamed(&self.header, &self.data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let (header, data) = decode_streamed(bytes)?;
        Ok(Self::new(header, data))
    }

    fn parts(&self) -> Part<'_> {
        let header = encode_streamed(&self.header, &[]);
        let payload = match self.saved {
            Some(saved) => Part::Reused(saved),
            None => Part::from(self.data.as_slice()),
        };
        Part::node(vec![Part::from(header), payload])
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let mut blob = Self::decode(&stored.bytes)?;
        let header = (stored.bytes.len() - blob.data.len()) as u64;
        let Some(payload) = stored
            .root
            .as_ref()
            .filter(|root| root.children.len() == 2)
            .map(|root| &root.children[1])
            .filter(|payload| payload.range().start == header)
        else {
            return Ok(blob);
        };
        blob.saved = Some(payload.reference);
        blob.unloaded = !stored.holds(payload);
        Ok(blob)
    }

    fn name(&self) -> Option<String> {
        let name = K::name(&self.header).trim();
        (!name.is_empty()).then(|| name.to_owned())
    }

    fn describe(&self, metadata: &mut BlockMetadata) {
        K::describe(&self.header, metadata);
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
        Self::new(header, payload)
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
