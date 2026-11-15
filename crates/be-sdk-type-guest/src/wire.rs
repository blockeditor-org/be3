use be_block_commit::CommitId;
use be_block_session::{Payload, SessionMessage};
use be_sdk_type_api::RelayKind;
use serde::{Deserialize, Serialize};

/// What a replica puts in the order, sealed as a submission's payload: the entry, and `through`,
/// how many local edits of its replica (counted across the replica's reopenings) the entry
/// holds with those before it, so a reopened replica skips the edits it is given again that
/// the order holds already.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Submitted {
    pub through: u64,
    pub entry: Entry,
}

/// The session state a journal's base and a reload carry (`JournalBase::state`), which every
/// replica opening there adopts: the type's own (`LiveEdit::session_state`), and how far the
/// order holds each recent origin's local edits (`Submitted::through`), so a replica opened
/// again there knows its own though the entries that held them were trimmed. Sealed with the
/// block's key like the rest of the base: the server reads none of it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BaseState {
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>,
    /// Origin, its highest `through`, and the sequence that holds it.
    pub origins: Vec<(u64, u64, u64)>,
    /// How many times the session started over before this state.
    pub restarts: u64,
}

impl BaseState {
    /// How many origins it keeps, the most recently numbered.
    pub const KEPT_ORIGINS: usize = 64;

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    /// None reads as no state; bytes that do not decode as none.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes.is_empty() {
            true => Some(Self::default()),
            false => postcard::from_bytes(bytes).ok(),
        }
    }
}

/// One entry of a session's order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Entry {
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
    pub fn encode(&self) -> Vec<u8> {
        be_block_protocol::encode(self).expect("entries always encode")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_block_protocol::decode(bytes).ok()
    }
}

/// What the server checks before relaying `message`: who may send it.
pub fn kind(message: &SessionMessage) -> RelayKind {
    match message {
        SessionMessage::Presence { .. } | SessionMessage::Channel { .. } => RelayKind::Peer,
        SessionMessage::Sealed { .. } => RelayKind::Seal,
    }
}

/// Whether a message the server relayed as `kind` may say this: what the server checked covers
/// what it asks for. A viewer could label a `Sealed` as presence; it is dropped.
pub fn sent_as(message: &SessionMessage, as_kind: RelayKind) -> bool {
    as_kind == RelayKind::Seal || kind(message) == RelayKind::Peer
}

pub fn encode(message: &SessionMessage) -> Vec<u8> {
    be_block_protocol::encode(message).expect("session messages always encode")
}

pub fn decode(bytes: &[u8]) -> Option<SessionMessage> {
    be_block_protocol::decode(bytes).ok()
}
