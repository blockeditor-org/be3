use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

be_ops_bindgen::generate!({
    path: "wit",
    world: "client",
    deps: ["../be-ops/wit"],
    network: true,
    with: {
        "uuid": uuid::Uuid,
        "hash": be_store::Hash,
        "commit-id": be_commit::CommitId,
        "public-key": [u8; 32],
        "access-level": be_graph::Access,
        "block-parent": be_graph::BlockParent,
        "escrow-key": crate::EscrowKey,
    },
    copy: ["uuid", "hash", "commit-id", "public-key", "access-level", "block-parent"],
    default: ["key-set", "key-work", "session-state", "access-list", "organization-kind"],
    inline: ["block-removed", "head-moved", "joined", "relayed"],
});

pub use account::*;
pub use blocks::*;
pub use contacts::*;
pub use keys::*;
pub use members::*;
pub use organizations::*;
pub use sessions::*;
pub use types::*;

pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

impl OrganizationRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::Editor => "Editor",
        }
    }
}

impl BlockSummary {
    pub fn no_access(id: Uuid, content_type: Uuid, access: be_graph::Access) -> Self {
        Self {
            id,
            content_type,
            access,
            author: None,
            parent: None,
            head: None,
            references: Vec::new(),
            metadata: Vec::new(),
            version: None,
            key: None,
            scope: None,
            revision: 0,
            held: true,
            opens_inside: false,
            sets_access: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EscrowKey {
    pub id: Uuid,
    pub epoch: u64,
    pub public: [u8; 32],
    pub signing: [u8; 32],
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl EscrowKey {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an escrow key always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    pub fn chained(&self, organization: Uuid, previous: Uuid) -> Vec<u8> {
        [
            b"be3.escrow.chain.v1".as_slice(),
            organization.as_bytes(),
            previous.as_bytes(),
            self.id.as_bytes(),
            &self.epoch.to_be_bytes(),
            &self.public,
            &self.signing,
        ]
        .concat()
    }
}

impl KeySet {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.wrapped.is_empty()
    }

    pub fn extend(&mut self, other: KeySet) {
        self.keys.extend(other.keys);
        self.wrapped.extend(other.wrapped);
    }
}

impl SessionState {
    pub fn is_owner(&self, client: ClientId) -> bool {
        self.owner == Some(client)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the server refused ({}): {}",
            self.code, self.message
        )
    }
}

impl std::error::Error for Error {}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(message)
}

pub fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

const HELLO: &[u8] = b"be3 hello\n";

pub fn hello(build: &str) -> Vec<u8> {
    [HELLO, build.as_bytes()].concat()
}

pub fn read_hello(frame: &[u8]) -> Option<String> {
    let build = frame.strip_prefix(HELLO)?;
    String::from_utf8(build.to_vec()).ok()
}

#[cfg(test)]
mod tests;
