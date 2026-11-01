use std::collections::BTreeMap;

pub use be_commit::CommitId;
pub use be_ops::{self as ops, FrameError, OpError, OpId, decode_frame, encode_frame};
pub use be_protocol::{ClientId, SessionState};
pub use be_session::Resume;
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

be_ops_bindgen::generate!({
    path: "wit",
    world: "type-module",
    deps: ["../be-ops/wit"],
    with: {
        "uuid": uuid::Uuid,
        "hash": [u8; 32],
        "commit-id": be_commit::CommitId,
        "session-tag": crate::SessionTag,
        "session-state": be_protocol::SessionState,
        "resume": be_session::Resume,
        "entries": crate::Entries,
    },
    encodings: {
        "entries": ::be_ops::wire::Map<::be_ops::wire::Plain, ::be_ops::wire::Bytes>,
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
