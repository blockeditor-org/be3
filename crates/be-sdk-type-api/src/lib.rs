use std::collections::BTreeMap;

pub use be_block_commit::CommitId;
pub use be_block_protocol::{ClientId, RelayKind, SessionState};
pub use be_block_session::Resume;
pub use be_wasm_ops::{self as ops, FrameError, OpError, OpId, decode_frame, encode_frame};
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "type-module",
    deps: ["../be-wasm-ops/wit"],
    with: {
        "uuid": uuid::Uuid,
        "hash": [u8; 32],
        "commit-id": be_block_commit::CommitId,
        "session-tag": crate::SessionTag,
        "session-state": be_block_protocol::SessionState,
        "relay-kind": be_block_protocol::RelayKind,
        "resume": be_block_session::Resume,
        "entries": crate::Entries,
    },
    encodings: {
        "entries": ::be_wasm_ops::wire::Map<::be_wasm_ops::wire::Plain, ::be_wasm_ops::wire::Bytes>,
    },
    copy: ["uuid", "hash", "commit-id", "session-tag", "relay-kind"],
    default: ["plain", "description", "plain-file", "status"],
});

pub use host::PublishOutcome;
pub use replicas::{KeptCommits, Opening, SealedHead};
pub use types::*;

pub type ReplicaId = OpId;

impl From<be_block_store::PartRef> for PartRef {
    fn from(part: be_block_store::PartRef) -> Self {
        let node = |node: be_block_store::NodeRef| NodeRef {
            hash: *node.hash.as_bytes(),
            length: node.length,
        };
        match part {
            be_block_store::PartRef::Chunk(chunk) => Self::Chunk(ChunkRef {
                hash: *chunk.hash.as_bytes(),
                length: chunk.length,
            }),
            be_block_store::PartRef::Chunks(chunks) => Self::Chunks(node(chunks)),
            be_block_store::PartRef::Node(parts) => Self::Node(node(parts)),
        }
    }
}

impl From<PartRef> for be_block_store::PartRef {
    fn from(part: PartRef) -> Self {
        let node = |node: NodeRef| be_block_store::NodeRef {
            hash: be_block_store::Hash::from_bytes(node.hash),
            length: node.length,
        };
        match part {
            PartRef::Chunk(chunk) => Self::Chunk(be_block_store::ChunkRef {
                hash: be_block_store::Hash::from_bytes(chunk.hash),
                length: chunk.length,
            }),
            PartRef::Chunks(chunks) => Self::Chunks(node(chunks)),
            PartRef::Node(parts) => Self::Node(node(parts)),
        }
    }
}

impl Loaded {
    pub fn of(stored: be_block_store::Stored) -> Self {
        fn flatten(part: &be_block_store::LoadedPart, parts: &mut Vec<LoadedPart>) {
            parts.push(LoadedPart {
                part: part.reference.into(),
                children: u32::try_from(part.children.len()).unwrap_or(u32::MAX),
                references: part.references.clone(),
            });
            for child in &part.children {
                flatten(child, parts);
            }
        }
        let mut parts = Vec::new();
        if let Some(root) = &stored.root {
            flatten(root, &mut parts);
        }
        Self {
            bytes: stored.bytes,
            parts,
        }
    }

    /// The `Stored` this was made from; none when its parts don't form one tree.
    pub fn into_stored(self) -> Option<be_block_store::Stored> {
        fn rebuild(
            parts: &mut std::slice::Iter<'_, LoadedPart>,
            start: u64,
            depth: usize,
        ) -> Option<be_block_store::LoadedPart> {
            let part = parts.next()?;
            let reference = be_block_store::PartRef::from(part.part);
            let mut children = Vec::new();
            let mut at = start;
            for _ in 0..part.children {
                if depth > 64 {
                    return None;
                }
                let child = rebuild(parts, at, depth + 1)?;
                at = child.range().end;
                children.push(child);
            }
            if !children.is_empty() && at != start + reference.length() {
                return None;
            }
            Some(be_block_store::LoadedPart {
                reference,
                start,
                children,
                references: part.references.clone(),
            })
        }
        let mut parts = self.parts.iter();
        let root = match self.parts.is_empty() {
            true => None,
            false => Some(rebuild(&mut parts, 0, 0)?),
        };
        if parts.next().is_some() {
            return None;
        }
        Some(be_block_store::Stored {
            bytes: self.bytes,
            root,
        })
    }
}

pub type Entries = BTreeMap<Uuid, Vec<u8>>;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct SessionTag(pub [u8; 32]);

/// Content's parts as `Written` carries them: the bytes of its new parts, and its
/// shape in pre-order, a `Reused` part kept by its ref. None only for a part
/// too long to describe.
pub fn written_shape(part: &be_block_store::Part<'_>) -> Option<(Vec<u8>, Vec<WrittenPart>)> {
    let mut bytes = Vec::new();
    let mut shape = Vec::new();
    shape_into(part, &mut bytes, &mut shape)?;
    Some((bytes, shape))
}

fn shape_into(
    part: &be_block_store::Part<'_>,
    bytes: &mut Vec<u8>,
    shape: &mut Vec<WrittenPart>,
) -> Option<()> {
    match part {
        be_block_store::Part::Bytes(held) => {
            shape.push(WrittenPart::Bytes(u32::try_from(held.len()).ok()?));
            bytes.extend_from_slice(held);
        }
        be_block_store::Part::Reused(part) => shape.push(WrittenPart::Reused((*part).into())),
        be_block_store::Part::Node { parts, references } => {
            shape.push(WrittenPart::Node(WrittenNode {
                parts: u32::try_from(parts.len()).ok()?,
                references: references.clone(),
            }));
            for part in parts {
                shape_into(part, bytes, shape)?;
            }
        }
    }
    Some(())
}

/// The parts to save `Written`'s bytes in, from its shape; one part when the
/// shape is empty or doesn't match the bytes.
pub fn written_parts<'a>(bytes: &'a [u8], shape: &[WrittenPart]) -> be_block_store::Part<'a> {
    let mut at = 0;
    let mut shape = shape.iter();
    match take_part(bytes, &mut at, &mut shape, 0) {
        Some(part) if at == bytes.len() && shape.next().is_none() => part,
        _ => be_block_store::Part::from(bytes),
    }
}

fn take_part<'a>(
    bytes: &'a [u8],
    at: &mut usize,
    shape: &mut std::slice::Iter<'_, WrittenPart>,
    depth: usize,
) -> Option<be_block_store::Part<'a>> {
    match shape.next()? {
        WrittenPart::Bytes(length) => {
            let end = at.checked_add(usize::try_from(*length).ok()?)?;
            let part = bytes.get(*at..end)?;
            *at = end;
            Some(be_block_store::Part::from(part))
        }
        WrittenPart::Node(node) if depth < 64 => {
            let mut parts = Vec::new();
            for _ in 0..node.parts {
                parts.push(take_part(bytes, at, shape, depth + 1)?);
            }
            Some(be_block_store::Part::Node {
                parts,
                references: node.references.clone(),
            })
        }
        WrittenPart::Node(_) => None,
        WrittenPart::Reused(part) => Some(be_block_store::Part::Reused((*part).into())),
    }
}
