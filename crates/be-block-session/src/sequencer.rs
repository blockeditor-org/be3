use std::collections::{HashSet, VecDeque};

use be_block_commit::CommitId;
use be_block_protocol::ClientId;
use be_block_store::PartRef;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OpId {
    pub client: ClientId,
    pub counter: u64,
}

/// An operation as a session carries it: its encoded bytes, or for one too large to relay
/// (over `LARGEST_RELAYED_OPERATION` in be-sdk-type-guest) the part its sender put them as
/// (`put-part`), which a replica reads before it applies the operation.
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionOp {
    pub id: OpId,
    pub sequence: u64,
    pub payload: Payload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SessionMessage {
    Submit {
        id: OpId,
        base: u64,
        payload: Payload,
    },
    Accepted {
        op: SessionOp,
    },
    /// A follower asks for what it lacks: the operations after `since`, and the head and
    /// session state unless it already holds `base` as sealed at `sealed`.
    Catchup {
        since: u64,
        base: Option<CommitId>,
        sealed: u64,
    },
    Snapshot {
        head: Option<CommitId>,
        sealed: u64,
        sequence: u64,
        ops: Vec<SessionOp>,
        #[serde(with = "serde_bytes")]
        state: Vec<u8>,
    },
    /// The owner committed `head`, holding everything up to `sequence`. With `reload`,
    /// followers load `head` with that session state; with `restart`, the session started over
    /// from `head` loaded afresh, so what followers had pending moves onto it
    /// (`LiveEdit::carry_over`) and undo stops there.
    Sealed {
        head: CommitId,
        sequence: u64,
        #[serde(with = "serde_bytes")]
        reload: Option<Vec<u8>>,
        restart: bool,
    },
    Replaced {
        head: CommitId,
    },
    /// A follower kept edits it could not hand over as operations (what it had not saved when
    /// a replacement landed) as `commit` on `base`; the owner keeps it in history and answers
    /// `Saved`.
    Kept {
        commit: CommitId,
        base: Option<CommitId>,
    },
    Saved {
        commit: CommitId,
    },
    /// The owner could not read the part of a `Submit` it was sent: the follower submits what
    /// it has pending again.
    Resend,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Inherited {
    Continued,
    Duplicate,
    Renumbered,
    Unnumbered,
}

#[derive(Clone, Debug, Default)]
pub struct Sequencer {
    sequence: u64,
    head: Option<CommitId>,
    log: VecDeque<SessionOp>,
    accepted: HashSet<OpId>,
    inherited: u64,
}

impl Sequencer {
    pub fn new(head: Option<CommitId>) -> Self {
        Self::continuing(head, 0, 0)
    }

    pub fn continuing(head: Option<CommitId>, sequence: u64, unsealed: u64) -> Self {
        Self {
            sequence,
            head,
            log: VecDeque::new(),
            accepted: HashSet::new(),
            inherited: unsealed,
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn head(&self) -> Option<CommitId> {
        self.head
    }

    pub fn accept(&mut self, id: OpId, payload: Payload) -> Option<SessionOp> {
        if !self.accepted.insert(id) {
            return None;
        }
        self.sequence += 1;
        let op = SessionOp {
            id,
            sequence: self.sequence,
            payload,
        };
        self.log.push_back(op.clone());
        Some(op)
    }

    pub fn inherit(&mut self, op: &SessionOp) -> Inherited {
        if self.accepted.contains(&op.id) {
            return match self.log.iter().any(|held| held == op) {
                true => Inherited::Duplicate,
                false => Inherited::Renumbered,
            };
        }
        self.accepted.insert(op.id);
        if op.sequence != self.sequence + 1 {
            return Inherited::Unnumbered;
        }
        self.sequence = op.sequence;
        self.log.push_back(op.clone());
        Inherited::Continued
    }

    /// Takes an operation replayed from the journal, numbered as its owner numbered it: a
    /// folded entry skips the numbers it absorbed.
    pub fn replayed(&mut self, op: SessionOp) {
        self.accepted.insert(op.id);
        self.sequence = self.sequence.max(op.sequence);
        self.log.push_back(op);
    }

    pub fn since(&self, sequence: u64) -> Vec<SessionOp> {
        self.log
            .iter()
            .filter(|op| op.sequence > sequence)
            .cloned()
            .collect()
    }

    pub fn sealed(&mut self, head: CommitId) {
        self.head = Some(head);
        self.log.clear();
        self.inherited = 0;
    }

    pub fn inherited(&self) -> u64 {
        self.inherited
    }

    pub fn carry_unsealed(&mut self) {
        self.inherited += 1;
    }

    pub fn pending_operations(&self) -> usize {
        self.log.len() + self.inherited as usize
    }

    pub fn is_clean(&self) -> bool {
        self.log.is_empty() && self.inherited == 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Follower {
    client: ClientId,
    counter: u64,
    applied: u64,
    pending: VecDeque<(OpId, Payload)>,
}

impl Follower {
    pub fn new(client: ClientId) -> Self {
        Self::continuing(client, 0)
    }

    pub fn continuing(client: ClientId, sequence: u64) -> Self {
        Self {
            client,
            counter: sequence,
            applied: sequence,
            pending: VecDeque::new(),
        }
    }

    pub fn applied(&self) -> u64 {
        self.applied
    }

    pub fn set_applied(&mut self, sequence: u64) {
        self.applied = sequence;
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn submit(&mut self, payload: Payload) -> SessionMessage {
        self.counter += 1;
        let id = OpId {
            client: self.client,
            counter: self.counter,
        };
        self.pending.push_back((id, payload.clone()));
        SessionMessage::Submit {
            id,
            base: self.applied,
            payload,
        }
    }

    pub fn is_mine(&self, id: OpId) -> bool {
        id.client == self.client
    }

    pub fn front(&self) -> Option<OpId> {
        self.pending.front().map(|(id, _)| *id)
    }

    /// Puts back what `take_pending` took, rewritten.
    pub fn restore_pending(&mut self, pending: Vec<(OpId, Payload)>) {
        self.pending = pending.into();
    }

    pub fn take_pending(&mut self) -> Vec<(OpId, Payload)> {
        self.pending.drain(..).collect()
    }

    /// The parts pending operations are carried by.
    pub fn pending_parts(&self) -> Vec<PartRef> {
        self.pending
            .iter()
            .filter_map(|(_, payload)| payload.part())
            .collect()
    }

    pub fn payloads(&self) -> Vec<Payload> {
        self.pending
            .iter()
            .map(|(_, payload)| payload.clone())
            .collect()
    }

    pub fn accepted(&mut self, op: &SessionOp) -> bool {
        self.applied = self.applied.max(op.sequence);
        let mine = op.id.client == self.client;
        self.pending.retain(|(id, _)| *id != op.id);
        !mine
    }

    pub fn resynchronize(&mut self, sequence: u64) -> Vec<SessionMessage> {
        self.applied = sequence;
        self.pending
            .iter()
            .map(|(id, payload)| SessionMessage::Submit {
                id: *id,
                base: sequence,
                payload: payload.clone(),
            })
            .collect()
    }

    pub fn catchup(&self, base: Option<CommitId>, sealed: u64) -> SessionMessage {
        SessionMessage::Catchup {
            since: self.applied,
            base,
            sealed,
        }
    }
}
