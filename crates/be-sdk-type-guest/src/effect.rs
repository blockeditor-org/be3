use be_block_commit::CommitId;
use be_sdk_type_api::{
    Description, JournalBase, JournalEntry, Merging, Recipients, RelayKind, ReplicaId, Status,
    Written,
};

pub(crate) type Token = u64;

pub(crate) type RequestId = u64;

pub(crate) enum Effect {
    Send {
        replica: ReplicaId,
        to: Recipients,
        kind: RelayKind,
        bytes: Vec<u8>,
    },
    Publish {
        replica: ReplicaId,
        token: Token,
        expected: Option<CommitId>,
        merging: Option<Merging>,
        content: Written,
        /// The session's generation and the last sequence a seal holds.
        sealed_through: Option<(u64, u64)>,
    },
    Stash {
        replica: ReplicaId,
        token: Token,
        parent: Option<CommitId>,
        content: Written,
    },
    Load {
        replica: ReplicaId,
        token: Token,
        commit: CommitId,
        first_part: bool,
    },
    ReadPart {
        replica: ReplicaId,
        token: Token,
        part: be_block_content::PartRef,
        offset: u64,
        length: u64,
    },
    PutPart {
        replica: ReplicaId,
        token: Token,
        bytes: Vec<u8>,
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
    /// Announcements of edits made here that the server already holds.
    Announce {
        replica: ReplicaId,
        announcements: Vec<be_block_content::Announcement>,
    },
    ReportClean {
        replica: ReplicaId,
        generation: u64,
        clean_at: Option<CommitId>,
    },
    Append {
        replica: ReplicaId,
        generation: u64,
        entries: Vec<JournalEntry>,
        relay: bool,
        /// The first sequence number the entries hold, folded ones included.
        from: u64,
        /// The announcements of the edits made here among the entries, by sequence.
        announcements: Vec<(u64, be_block_content::Announcement)>,
    },
    Rebase {
        replica: ReplicaId,
        generation: u64,
        base: JournalBase,
        entries: Vec<JournalEntry>,
    },
    Release {
        replica: ReplicaId,
    },
    Status {
        replica: ReplicaId,
        status: Status,
    },
    Described {
        replica: ReplicaId,
        description: Description,
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
    Answer {
        id: RequestId,
        answer: Answer,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Answer {
    Unknown,
    Changed(bool),
    Content(Vec<u8>),
    Sealed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadEvent {
    Published {
        token: Token,
        head: CommitId,
        /// False for a commit kept on this device, which the server lacks until `drained`.
        held: bool,
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
    },
}
