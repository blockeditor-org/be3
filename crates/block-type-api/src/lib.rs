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
    with: {
        "uuid": uuid::Uuid,
        "commit-id": be_commit::CommitId,
        "session-tag": crate::SessionTag,
        "session-state": be_protocol::SessionState,
        "resume": be_session::Resume,
        "entries": crate::Entries,
    },
    copy: ["uuid", "commit-id", "session-tag"],
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

impl Default for Plain {
    fn default() -> Self {
        Plain::None
    }
}

impl Default for Description {
    fn default() -> Self {
        Self {
            name: None,
            derived: Entries::new(),
            references: Vec::new(),
        }
    }
}

impl Default for PlainFile {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            name: None,
            extension: String::new(),
            media_type: String::new(),
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Self {
            head: None,
            owner: false,
            clean: false,
            owes_seal: false,
            incompatible: false,
            diverged: false,
            unreadable: None,
        }
    }
}
