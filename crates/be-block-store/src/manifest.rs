use std::{borrow::Cow, ops::Range};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{StoreError, hash::Hash};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChunkRef {
    pub hash: Hash,
    pub length: u32,
}

/// A sealed `Node` and the length of the bytes under it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct NodeRef {
    pub hash: Hash,
    pub length: u64,
}

/// A saved part of a block's stored form, as a manifest names it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PartRef {
    /// A part's bytes in one chunk.
    Chunk(ChunkRef),
    /// A part's bytes split over several chunks: a node of `Chunk` and `Chunks` refs the vault
    /// grouped, which a reader sees as one part.
    Chunks(NodeRef),
    /// A part with child parts: a node of the children's refs, in order.
    Node(NodeRef),
}

impl PartRef {
    pub const fn hash(self) -> Hash {
        match self {
            Self::Chunk(chunk) => chunk.hash,
            Self::Chunks(node) | Self::Node(node) => node.hash,
        }
    }

    pub const fn length(self) -> u64 {
        match self {
            Self::Chunk(chunk) => chunk.length as u64,
            Self::Chunks(node) | Self::Node(node) => node.length,
        }
    }

    pub const fn node(self) -> Option<NodeRef> {
        match self {
            Self::Chunk(_) => None,
            Self::Chunks(node) | Self::Node(node) => Some(node),
        }
    }
}

/// One object of a manifest tree: the parts under it in order, whose bytes concatenated are
/// its bytes, and the blocks the content said this part references (`Part::Node`), which a
/// commit that reuses the node keeps without reading its bytes.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Node {
    pub parts: Vec<PartRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<Uuid>,
}

/// What a commit holds of its content: the root of a tree of sealed nodes over the chunks of
/// the stored form. A commit that changes one part costs that part's chunks and the nodes on
/// the path to it; every other subtree is shared with the commit it was loaded from.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub content_type: Uuid,
    pub root: Option<PartRef>,
}

impl Manifest {
    pub fn length(&self) -> u64 {
        self.root.map_or(0, PartRef::length)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
}

/// A block's stored form as content hands it to be saved: a tree whose leaves, in order, are
/// the stored bytes. `BlockContent::parts` returns one; by default it is one `Bytes` holding
/// `encode()`.
///
/// Each `Bytes` is sealed on its own, split by the vault's chunker when it is larger than the
/// chunker's minimum, so an edit to one part re-uploads only that part. `Reused` names a part
/// of the head the content was loaded from, which is kept as it was without sealing it again,
/// unless it was sealed under another data key (a rotation): then the vault seals its objects
/// again under the write's key. `Node` groups parts, and becomes a node of
/// the manifest tree, so a commit's new nodes are the ones on the path to what changed. Keep a
/// `Node`'s children few (dozens, not thousands): every change to one of them writes the node
/// again. Empty parts are left out.
///
/// A `Node`'s `references` are the blocks its own bytes reference (not its children's, which
/// say their own). A commit references what `BlockContent::references_in` returns and then
/// every node's references in tree order, so content that loads or re-encodes only some parts
/// keeps the references of the parts it reuses (guides/blocks/content_types.md, "Parts").
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Part<'a> {
    Bytes(Cow<'a, [u8]>),
    Reused(PartRef),
    Node {
        parts: Vec<Part<'a>>,
        references: Vec<Uuid>,
    },
}

impl<'a> Part<'a> {
    /// A node of `parts` that references nothing itself.
    pub const fn node(parts: Vec<Part<'a>>) -> Self {
        Self::Node {
            parts,
            references: Vec::new(),
        }
    }
}

impl Part<'_> {
    pub fn length(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::Reused(part) => part.length(),
            Self::Node { parts, .. } => parts.iter().map(Part::length).sum(),
        }
    }

    /// The stored form this tree describes, for a part that holds no `Reused` part.
    pub fn flatten(&self) -> Option<Vec<u8>> {
        let mut bytes = Vec::with_capacity(usize::try_from(self.length()).unwrap_or(0));
        self.flatten_into(&mut bytes).then_some(bytes)
    }

    fn flatten_into(&self, bytes: &mut Vec<u8>) -> bool {
        match self {
            Self::Bytes(held) => {
                bytes.extend_from_slice(held);
                true
            }
            Self::Reused(_) => false,
            Self::Node { parts, .. } => parts.iter().all(|part| part.flatten_into(bytes)),
        }
    }
}

impl<'a> From<&'a [u8]> for Part<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self::Bytes(Cow::Borrowed(bytes))
    }
}

impl<'a> From<&'a Vec<u8>> for Part<'a> {
    fn from(bytes: &'a Vec<u8>) -> Self {
        Self::Bytes(Cow::Borrowed(bytes))
    }
}

impl From<Vec<u8>> for Part<'_> {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(Cow::Owned(bytes))
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Part<'a> {
    fn from(bytes: &'a [u8; N]) -> Self {
        Self::Bytes(Cow::Borrowed(bytes))
    }
}

/// A saved part as a reader loaded it: where its bytes are in the stored form, and its child
/// parts in the shape the content wrote them (`PartRef::Node`). A part whose bytes the vault
/// split over chunks is one part with no children. `references` are the node's own
/// (`Part::Node`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedPart {
    pub reference: PartRef,
    pub start: u64,
    pub children: Vec<LoadedPart>,
    pub references: Vec<Uuid>,
}

impl LoadedPart {
    pub fn range(&self) -> Range<u64> {
        self.start..self.start + self.reference.length()
    }

    /// Where the first child part ends, or this part when it has none: what a replica that
    /// loads only the first part reads.
    pub fn first_end(&self) -> u64 {
        match self.children.first() {
            Some(first) => first.range().end,
            None => self.range().end,
        }
    }
}

/// A block's stored form as read from a manifest: the bytes `BlockContent::decode` reads, and
/// the parts they were saved in, so content can hand unchanged ones back as `Part::Reused`.
/// Content that loads part of itself (`be_block_content::Loads::FirstPart`) gets only the stored
/// form's start in `bytes`; `holds` says which parts it has.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stored {
    pub bytes: Vec<u8>,
    pub root: Option<LoadedPart>,
}

impl Stored {
    /// Stored bytes with no saved parts behind them, as content is before its first save.
    pub fn unsaved(bytes: Vec<u8>) -> Self {
        Self { bytes, root: None }
    }

    /// Whether `bytes` holds all of `part`.
    pub fn holds(&self, part: &LoadedPart) -> bool {
        part.range().end <= self.bytes.len() as u64
    }

    /// The whole stored form's length, which `bytes` may hold only the start of.
    pub fn length(&self) -> u64 {
        self.root
            .as_ref()
            .map_or(self.bytes.len() as u64, |root| root.reference.length())
    }

    pub fn bytes_of(&self, part: &LoadedPart) -> &[u8] {
        let range = part.range();
        let start = usize::try_from(range.start).unwrap_or(usize::MAX);
        let end = usize::try_from(range.end).unwrap_or(usize::MAX);
        self.bytes.get(start..end).unwrap_or_default()
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
