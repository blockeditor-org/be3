use std::{collections::BTreeMap, fmt};

pub use be_commit::CommitId;
pub use be_protocol::{ClientId, SessionState};
pub use be_session::Resume;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub type ReplicaId = u64;

pub type RequestId = u64;

pub type Token = u64;

pub type EditorId = u64;

pub type Entries = BTreeMap<Uuid, Vec<u8>>;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct SessionTag(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Endpoint {
    Peer(ClientId),
    Editor(EditorId),
    Host,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Recipients {
    One(Endpoint),
    Peers,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Event {
    Entropy {
        seed: [u8; 32],
    },
    Tick {
        now: u64,
    },
    Open {
        replica: ReplicaId,
        block: Uuid,
        content_type: Uuid,
        organization: Uuid,
        client: ClientId,
        tag: SessionTag,
        head: Option<CommitId>,
        session: SessionState,
    },
    Message {
        replica: ReplicaId,
        from: Endpoint,
        bytes: Vec<u8>,
    },
    Attach {
        replica: ReplicaId,
        editor: EditorId,
        editable: bool,
    },
    Detach {
        replica: ReplicaId,
        editor: EditorId,
    },
    Head {
        replica: ReplicaId,
        event: HeadEvent,
    },
    Lease {
        replica: ReplicaId,
        session: SessionState,
        resynchronize: bool,
    },
    Read {
        replica: ReplicaId,
        token: Token,
        result: Result<Vec<u8>, String>,
    },
    Resumed {
        replica: ReplicaId,
        token: Token,
        result: Result<Resume, String>,
    },
    Close {
        replica: ReplicaId,
    },
    Request {
        id: RequestId,
        request: Request,
    },
    Claim {
        replica: ReplicaId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HeadEvent {
    Published {
        token: Token,
        head: CommitId,
    },
    Rejected {
        token: Token,
        head: Option<CommitId>,
    },
    Stashed {
        token: Token,
        commit: CommitId,
    },
    Failed {
        token: Token,
        error: String,
    },
    Moved {
        head: CommitId,
        from_this_device: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChildChange {
    Add(Uuid),
    Delete(Uuid),
    Replace { old: Uuid, new: Uuid },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Change {
    Child(ChildChange),
    FromFile { name: String, bytes: Vec<u8> },
    Replace(Vec<u8>),
    Undo,
    Redo,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Request {
    Blank {
        content_type: Uuid,
    },
    Merge3 {
        content_type: Uuid,
        base: Option<Vec<u8>>,
        ours: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
    },
    Describe {
        content_type: Uuid,
        bytes: Vec<u8>,
        organization: Uuid,
    },
    ToFile {
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Claims {
        name: String,
        first: Vec<u8>,
        directory: bool,
        fallback: bool,
    },
    Apply {
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    },
    Change {
        replica: ReplicaId,
        change: Change,
    },
    Content {
        replica: ReplicaId,
    },
    Seal {
        replica: ReplicaId,
        force: bool,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Description {
    pub name: Option<String>,
    pub derived: Entries,
    pub references: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Reply {
    Unknown,
    Malformed(String),
    Blank(Vec<u8>),
    Merged { bytes: Vec<u8>, conflicts: usize },
    Described(Description),
    File(Option<PlainFile>),
    Claimed(Option<Uuid>),
    Changed(bool),
    Content(Vec<u8>),
    Sealed,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlainFile {
    pub bytes: Vec<u8>,
    pub name: Option<String>,
    pub extension: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Written {
    pub bytes: Vec<u8>,
    pub references: Vec<Uuid>,
    pub derived: Entries,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Merging {
    pub base: Option<CommitId>,
    pub ours: CommitId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TypeInfo {
    pub content_type: Uuid,
    pub frozen: bool,
    pub schema: String,
    pub undo: bool,
    pub plain: Plain,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Plain {
    #[default]
    None,
    File,
    Directory,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Status {
    pub head: Option<CommitId>,
    pub owner: bool,
    pub clean: bool,
    pub owes_seal: bool,
    pub incompatible: bool,
    pub diverged: bool,
    pub unreadable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Command {
    Hello {
        types: Vec<TypeInfo>,
    },
    Send {
        replica: ReplicaId,
        to: Recipients,
        bytes: Vec<u8>,
    },
    Publish {
        replica: ReplicaId,
        token: Token,
        expected: Option<CommitId>,
        merging: Option<Merging>,
        content: Written,
    },
    Stash {
        replica: ReplicaId,
        token: Token,
        parent: Option<CommitId>,
        content: Written,
    },
    Read {
        replica: ReplicaId,
        token: Token,
        commit: CommitId,
        range: Option<ByteRange>,
    },
    Resume {
        replica: ReplicaId,
        token: Token,
        local: Option<CommitId>,
    },
    ClaimLease {
        replica: ReplicaId,
        generation: u64,
    },
    ReportClean {
        replica: ReplicaId,
        generation: u64,
        clean_at: Option<CommitId>,
    },
    Release {
        replica: ReplicaId,
    },
    WakeAt {
        at: u64,
    },
    Status {
        replica: ReplicaId,
        status: Status,
    },
    Described {
        replica: ReplicaId,
        description: Description,
    },
    History {
        replica: ReplicaId,
        can_undo: bool,
        can_redo: bool,
    },
    Conflict {
        replica: ReplicaId,
        ours: CommitId,
        theirs: CommitId,
        count: usize,
    },
    Error {
        replica: ReplicaId,
        message: String,
    },
    Reply {
        id: RequestId,
        reply: Reply,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    Truncated { expected: usize, available: usize },
    Malformed(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated {
                expected,
                available,
            } => write!(
                formatter,
                "a frame of {expected} bytes arrived with {available}"
            ),
            Self::Malformed(error) => write!(formatter, "a frame did not decode: {error}"),
        }
    }
}

impl std::error::Error for FrameError {}

pub fn encode_frame<T: Serialize>(message: &T) -> Vec<u8> {
    let payload = postcard::to_stdvec(message).expect("protocol messages always encode");
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

pub fn decode_frame<T: DeserializeOwned>(frame: &[u8]) -> Result<T, FrameError> {
    let Some((length, payload)) = frame.split_first_chunk::<4>() else {
        return Err(FrameError::Truncated {
            expected: 4,
            available: frame.len(),
        });
    };
    let length = u32::from_be_bytes(*length) as usize;
    if payload.len() != length {
        return Err(FrameError::Truncated {
            expected: length + 4,
            available: frame.len(),
        });
    }
    postcard::from_bytes(payload).map_err(|error| FrameError::Malformed(error.to_string()))
}

#[cfg(test)]
mod tests;
