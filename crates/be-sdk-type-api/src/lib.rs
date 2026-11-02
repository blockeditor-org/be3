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
