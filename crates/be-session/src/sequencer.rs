use std::collections::{BTreeMap, VecDeque};

use be_commit::CommitId;
pub use be_protocol::{OpId, SessionLog, SessionOp, SessionSeal};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SessionMessage {
    Sealed {
        head: CommitId,
        sequence: u64,
        kept: Option<CommitId>,
    },
    Presence {
        kind: uuid::Uuid,
        value: Option<Vec<u8>>,
    },
    Channel {
        channel: uuid::Uuid,
        payload: Vec<u8>,
    },
}

impl SessionMessage {
    pub fn encode(&self) -> Vec<u8> {
        be_protocol::encode(self).expect("session messages always encode")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_protocol::decode(bytes).ok()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sequencer {
    sequence: u64,
    seal: SessionSeal,
    start: u64,
    barrier: u64,
    log: VecDeque<SessionOp>,
    counters: BTreeMap<u64, u64>,
}

impl Sequencer {
    pub fn new(head: Option<CommitId>) -> Self {
        Self {
            seal: SessionSeal {
                head,
                sequence: 0,
                state: None,
            },
            ..Self::default()
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn seal(&self) -> &SessionSeal {
        &self.seal
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn floor(&self) -> u64 {
        self.start.max(self.barrier)
    }

    pub fn submit(
        &mut self,
        id: OpId,
        base: u64,
        barrier: bool,
        payload: Vec<u8>,
    ) -> Option<SessionOp> {
        let last = self.counters.entry(id.origin).or_default();
        if id.counter <= *last || base > self.sequence {
            return None;
        }
        *last = id.counter;
        let floor = self.floor();
        self.sequence += 1;
        if barrier {
            self.barrier = self.sequence;
        }
        let op = SessionOp {
            id,
            sequence: self.sequence,
            base,
            floor,
            payload,
        };
        self.log.push_back(op.clone());
        Some(op)
    }

    pub fn sealed(&mut self, seal: SessionSeal) -> bool {
        if seal.sequence > self.sequence || seal.sequence < self.seal.sequence {
            return false;
        }
        self.seal = seal;
        let mut start = self.seal.sequence;
        while let Some(base) = self
            .log
            .iter()
            .filter(|op| op.sequence > start && op.base >= op.floor)
            .map(|op| op.base)
            .min()
            .filter(|base| *base < start)
        {
            start = base;
        }
        self.start = self.start.max(start);
        let start = self.start;
        self.log.retain(|op| op.sequence > start);
        true
    }

    pub fn log(&self, origin: u64) -> SessionLog {
        SessionLog {
            seal: self.seal.clone(),
            ops: self.log.iter().cloned().collect(),
            counter: self.counters.get(&origin).copied().unwrap_or_default(),
        }
    }

    pub fn held(&self) -> usize {
        self.log.len()
    }

    pub fn is_clean(&self) -> bool {
        self.seal.sequence == self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Submission {
    pub id: OpId,
    pub base: u64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct Follower {
    origin: u64,
    counter: u64,
    applied: u64,
    pending: VecDeque<(OpId, Vec<u8>)>,
}

impl Follower {
    pub fn new(origin: u64) -> Self {
        Self {
            origin,
            ..Self::default()
        }
    }

    pub fn origin(&self) -> u64 {
        self.origin
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

    pub fn next_id(&mut self) -> OpId {
        self.counter += 1;
        OpId {
            origin: self.origin,
            counter: self.counter,
        }
    }

    pub fn submit(&mut self, payload: Vec<u8>) -> Submission {
        let id = self.next_id();
        self.pending.push_back((id, payload.clone()));
        Submission {
            id,
            base: self.applied,
            payload,
        }
    }

    pub fn is_mine(&self, id: OpId) -> bool {
        id.origin == self.origin
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

    pub fn accepted(&mut self, id: OpId, sequence: u64) -> bool {
        self.applied = self.applied.max(sequence);
        self.pending.retain(|(held, _)| *held != id);
        !self.is_mine(id)
    }

    pub fn forget(&mut self, id: OpId) -> Option<Vec<u8>> {
        let index = self.pending.iter().position(|(held, _)| *held == id)?;
        self.pending.remove(index).map(|(_, payload)| payload)
    }

    pub fn sequenced_through(&mut self, counter: u64) {
        self.pending.retain(|(id, _)| id.counter > counter);
    }

    pub fn resynchronize(&self) -> Vec<Submission> {
        self.pending
            .iter()
            .map(|(id, payload)| Submission {
                id: *id,
                base: self.applied,
                payload: payload.clone(),
            })
            .collect()
    }
}
