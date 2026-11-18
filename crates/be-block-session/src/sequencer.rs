use be_block_store::PartRef;
use serde::{Deserialize, Serialize};

/// A submission's id: the replica that made it (`origin`, random per replica and kept across
/// its reopenings) and a counter that replica never repeats.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OpId {
    pub origin: u64,
    pub counter: u64,
}

/// An operation as a session carries it: its encoded bytes, or for one too large to submit
/// inline (over `LARGEST_RELAYED_OPERATION` in be-sdk-type-guest) the part its sender put them
/// as (`put-part`), which a replica reads before it applies the operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Payload {
    Inline(#[serde(with = "serde_bytes")] Vec<u8>),
    Part(PartRef),
}

impl Payload {
    /// The part this payload must be read from, if it is not inline.
    pub fn part(&self) -> Option<PartRef> {
        match self {
            Self::Inline(_) => None,
            Self::Part(part) => Some(*part),
        }
    }

    /// The length of the operation's bytes, wherever they are.
    pub fn length(&self) -> u64 {
        match self {
            Self::Inline(bytes) => bytes.len() as u64,
            Self::Part(part) => part.length(),
        }
    }
}

/// What a replica hands the order: its id, the last sequence number the replica had applied
/// when it made it (`base`), whether it is a barrier (a replacement or a restart, after which
/// nothing based before it applies), and the sealed payload, which the server never opens.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Submission {
    pub id: OpId,
    pub base: u64,
    pub barrier: bool,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

/// A submission as the order numbered it: `sequence` is its place, `floor` the lowest `base`
/// still applied (the last barrier: an operation based below it is passed over by every replica
/// alike), and
/// `received` when the server took it, by its clock (none for an order kept offline).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionOp {
    pub id: OpId,
    pub sequence: u64,
    pub base: u64,
    pub floor: u64,
    pub received: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

/// What replicas relay to each other outside the order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SessionMessage {
    Presence {
        kind: uuid::Uuid,
        #[serde(with = "serde_bytes")]
        value: Option<Vec<u8>>,
    },
    Channel {
        channel: uuid::Uuid,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
}

/// Where a session's order stands: the server's half of a live session, kept beside the block's
/// journal (and on a device offline beside its own journal). It numbers each submission and
/// keeps the floor.
///
/// - `sequence`: the last number given.
/// - `barrier`: the last barrier's number; nothing based before it applies.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Sequencer {
    pub sequence: u64,
    pub barrier: u64,
}

impl Sequencer {
    /// An order that starts after `sequence`, as a journal's base holds it.
    pub fn starting(sequence: u64) -> Self {
        Self {
            sequence,
            barrier: 0,
        }
    }

    /// The floor an operation numbered now carries.
    pub fn floor(&self) -> u64 {
        self.barrier
    }

    /// Numbers `submission`, taken at `received`.
    pub fn number(&mut self, submission: Submission, received: Option<i64>) -> SessionOp {
        self.sequence += 1;
        if submission.barrier {
            self.barrier = self.sequence;
        }
        SessionOp {
            id: submission.id,
            sequence: self.sequence,
            base: submission.base,
            floor: self.floor(),
            received,
            payload: submission.payload,
        }
    }
}

/// Whether a journal holds exactly what a seal holds: its base is at or before the seal's
/// sequence, and it reaches the seal's last entry. `entries` are each entry's sequence, in
/// order. A journal with no entry through the seal holds nothing of it, unless its base is the
/// seal's sequence.
pub fn reaches_seal(
    base_sequence: u64,
    entries: impl IntoIterator<Item = u64>,
    through: u64,
) -> bool {
    base_sequence <= through
        && (base_sequence == through || entries.into_iter().any(|sequence| sequence == through))
}
