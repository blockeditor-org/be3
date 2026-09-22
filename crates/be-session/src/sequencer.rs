use std::collections::{HashSet, VecDeque};

use be_commit::CommitId;
use be_protocol::ClientId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OpId {
    pub client: ClientId,
    pub counter: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionOp {
    pub id: OpId,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SessionMessage {
    Submit {
        id: OpId,
        base: u64,
        payload: Vec<u8>,
    },
    Accepted {
        op: SessionOp,
    },
    Catchup {
        since: u64,
    },
    Snapshot {
        head: Option<CommitId>,
        sequence: u64,
        ops: Vec<SessionOp>,
    },
    Sealed {
        head: CommitId,
        sequence: u64,
        reload: bool,
    },
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

    pub fn accept(&mut self, id: OpId, payload: Vec<u8>) -> Option<SessionOp> {
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

    pub fn since(&self, sequence: u64) -> Vec<SessionOp> {
        self.log
            .iter()
            .filter(|op| op.sequence > sequence)
            .cloned()
            .collect()
    }

    pub fn snapshot(&self) -> SessionMessage {
        SessionMessage::Snapshot {
            head: self.head,
            sequence: self.sequence,
            ops: self.log.iter().cloned().collect(),
        }
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
    pending: VecDeque<(OpId, Vec<u8>)>,
}

impl Follower {
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            counter: 0,
            applied: 0,
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

    pub fn submit(&mut self, payload: Vec<u8>) -> SessionMessage {
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

    pub fn rewrite_pending(&mut self, rewrite: impl Fn(&[u8]) -> Option<Vec<u8>>) {
        let mut kept = VecDeque::new();
        while let Some((id, payload)) = self.pending.pop_front() {
            if let Some(payload) = rewrite(&payload) {
                kept.push_back((id, payload));
            }
        }
        self.pending = kept;
    }

    pub fn take_pending(&mut self) -> Vec<(OpId, Vec<u8>)> {
        self.pending.drain(..).collect()
    }

    pub fn payloads(&self) -> Vec<Vec<u8>> {
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

    pub fn catchup(&self) -> SessionMessage {
        SessionMessage::Catchup {
            since: self.applied,
        }
    }
}
