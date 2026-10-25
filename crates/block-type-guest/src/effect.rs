use be_commit::CommitId;
use block_type_api::{Description, Merging, Recipients, ReplicaId, Status, Written};

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
