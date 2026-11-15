use be_block_commit::CommitId;
use be_block_session::{Payload, SessionMessage};
use be_sdk_type_api::RelayKind;
use serde::{Deserialize, Serialize};

/// What a replica puts in the order, sealed as a submission's payload: the entry, and `through`,
/// how many local edits of its replica (counted across the replica's reopenings) the entry
/// holds with those before it, so a reopened replica skips the edits it is given again that
/// the order holds already.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Submitted {
    pub(crate) through: u64,
    pub(crate) entry: Entry,
}

/// One entry of a session's order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum Entry {
    /// An operation, applied by every replica at its place.
    Operation(Payload),
    /// The content becomes `commit` (with `state` adopted when there is one) at this place:
    /// without `replace` every replica then applies again the operations numbered after the
    /// entry's base, which the commit lacks; with it, none, and it is a barrier. With `head`
    /// the commit is the block's head (the lease holder merged a head moved from elsewhere);
    /// otherwise it is a stash every replica remembers until the holder seals it as a second
    /// parent, so its history stays. With `restart` the session starts over from the commit
    /// loaded afresh: every replica moves what it has not had numbered onto it
    /// (`LiveEdit::carry_over`) and undo stops there.
    Reload {
        commit: CommitId,
        head: bool,
        replace: bool,
        restart: bool,
        #[serde(with = "serde_bytes")]
        state: Vec<u8>,
    },
    /// A stash to keep in history: the holder's next seal has it as a second parent.
    Keep { commit: CommitId },
}

impl Submitted {
    pub(crate) fn encode(&self) -> Vec<u8> {
        be_block_protocol::encode(self).expect("entries always encode")
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        be_block_protocol::decode(bytes).ok()
    }
}

/// What the server checks before relaying `message`: who may send it.
pub(crate) fn kind(message: &SessionMessage) -> RelayKind {
    match message {
        SessionMessage::Presence { .. } | SessionMessage::Channel { .. } => RelayKind::Peer,
        SessionMessage::Sealed { .. } => RelayKind::Edit,
    }
}

/// Whether a message the server relayed as `kind` may say this: what the server checked covers
/// what it asks for. A viewer could label a `Sealed` as presence; it is dropped.
pub(crate) fn sent_as(message: &SessionMessage, as_kind: RelayKind) -> bool {
    as_kind == RelayKind::Edit || kind(message) == RelayKind::Peer
}

pub(crate) fn encode(message: &SessionMessage) -> Vec<u8> {
    be_block_protocol::encode(message).expect("session messages always encode")
}

pub(crate) fn decode(bytes: &[u8]) -> Option<SessionMessage> {
    be_block_protocol::decode(bytes).ok()
}
