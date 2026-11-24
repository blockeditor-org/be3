use std::fmt::Debug;

use be_block_commit::MergeResult;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    BlockContent, BlockMetadata, ContentError, LiveEdit, Loads, Merge, Part, PartRef, Stored,
};

/// The stored form's prefix: the header's length, as a little-endian `u32`.
const HEADER_PREFIX_BYTES: usize = 4;

pub trait BlobKind: Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    const FROZEN: bool = false;

    const SCHEMA: u32 = 0;

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

    /// False for a blob a replica loaded, which holds only its header: its payload stays in
    /// the store, and editors read it by range (`ContentProjection::payload` in be-sdk-beui).
    pub const fn is_loaded(&self) -> bool {
        !self.unloaded
    }

    /// Where the payload starts in the stored form, after the encoded header: a reader of a
    /// blob loaded without its payload reads the payload's bytes from here on.
    pub fn payload_offset(&self) -> u64 {
        encode_streamed(&self.header, &[]).len() as u64
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

    const LOADS: Loads = Loads::FirstPart;

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

/// `[u32 header length][header][payload]`, so a reader takes the header from the start.
fn encode_streamed<H: Serialize>(header: &H, payload: &[u8]) -> Vec<u8> {
    let encoded = postcard::to_stdvec(header).unwrap_or_default();
    let length = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(HEADER_PREFIX_BYTES + encoded.len() + payload.len());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&encoded);
    bytes.extend_from_slice(payload);
    bytes
}

fn decode_streamed<H: DeserializeOwned>(bytes: &[u8]) -> Result<(H, Vec<u8>), ContentError> {
    let start = payload_start(bytes)?;
    if bytes.len() < start {
        return Err(ContentError::Truncated);
    }
    let header = postcard::from_bytes(&bytes[HEADER_PREFIX_BYTES..start])
        .map_err(|_| ContentError::Malformed("header"))?;
    Ok((header, bytes[start..].to_vec()))
}

fn payload_start(prefix: &[u8]) -> Result<usize, ContentError> {
    let length = prefix
        .get(..HEADER_PREFIX_BYTES)
        .ok_or(ContentError::Truncated)?;
    let length = u32::from_le_bytes(length.try_into().expect("four bytes")) as usize;
    HEADER_PREFIX_BYTES
        .checked_add(length)
        .ok_or(ContentError::Malformed("header length"))
}
