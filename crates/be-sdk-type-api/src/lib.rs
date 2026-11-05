use std::collections::BTreeMap;

pub use be_block_commit::CommitId;
pub use be_block_protocol::{ClientId, SessionState};
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
        "resume": be_block_session::Resume,
        "entries": crate::Entries,
    },
    encodings: {
        "entries": ::be_wasm_ops::wire::Map<::be_wasm_ops::wire::Plain, ::be_wasm_ops::wire::Bytes>,
    },
    copy: ["uuid", "hash", "commit-id", "session-tag"],
    default: ["plain", "description", "plain-file", "status"],
});

pub use host::PublishOutcome;
pub use replicas::Opening;
pub use types::*;

pub type ReplicaId = OpId;

pub type Entries = BTreeMap<Uuid, Vec<u8>>;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct SessionTag(pub [u8; 32]);

/// Content's parts as `Written` carries them: their bytes, and their shape in
/// pre-order. None when a part is `Reused`, whose bytes a replica doesn't hold.
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
        be_block_store::Part::Reused(_) => return None,
        be_block_store::Part::Node(parts) => {
            shape.push(WrittenPart::Node(u32::try_from(parts.len()).ok()?));
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
        WrittenPart::Node(count) if depth < 64 => {
            let mut parts = Vec::new();
            for _ in 0..*count {
                parts.push(take_part(bytes, at, shape, depth + 1)?);
            }
            Some(be_block_store::Part::Node(parts))
        }
        WrittenPart::Node(_) => None,
    }
}
