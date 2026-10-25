use be_commit::CommitId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Entry {
    Operation(Vec<u8>),
    Reload {
        commit: CommitId,
        head: bool,
        replace: bool,
        state: Option<Vec<u8>>,
    },
    Keep {
        commit: CommitId,
    },
}

impl Entry {
    pub fn encode(&self) -> Vec<u8> {
        be_protocol::encode(self).expect("session entries always encode")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_protocol::decode(bytes).ok()
    }
}
