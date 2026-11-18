use be_block_commit::CommitId;
use be_sdk_type_api::{Description, JournalBase, Merging, Recipients, ReplicaId, Status, Written};

pub(crate) type Token = u64;

pub(crate) type RequestId = u64;

pub(crate) enum Effect {
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
        /// The last sequence of the order a seal holds, and the run of the order it is in.
        sealed_through: Option<u64>,
        /// With `sealed_through`, the session state the journal starts from with the commit.
        journal_state: Option<Vec<u8>>,
        restarts: u64,
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
    Append {
        replica: ReplicaId,
        token: Token,
        bytes: Vec<u8>,
    },
    ReadLog {
        replica: ReplicaId,
        token: Token,
        from: u64,
        limit: u32,
    },
    Resume {
        replica: ReplicaId,
        token: Token,
        local: Option<CommitId>,
    },
    Claim {
        replica: ReplicaId,
    },
    Submit {
        replica: ReplicaId,
        submission: be_block_session::Submission,
        /// What the edits it holds announce.
        announcements: Vec<be_block_content::Announcement>,
    },
    Rebase {
        replica: ReplicaId,
        base: JournalBase,
        /// The run of the order it is in (`status.restarts`).
        restarts: u64,
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
