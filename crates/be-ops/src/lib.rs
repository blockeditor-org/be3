use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    marker::PhantomData,
};

use serde::Deserialize;
pub use serde::{Serialize, de::DeserializeOwned};

pub mod guest;

pub type OpId = u64;

pub type MethodId = u32;

pub const GUEST_BIT: OpId = 1 << 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Host,
    Guest,
}

impl Side {
    fn bit(self) -> OpId {
        match self {
            Side::Host => 0,
            Side::Guest => GUEST_BIT,
        }
    }

    pub fn started(self, op: OpId) -> bool {
        op & GUEST_BIT == self.bit()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Frame {
    Start {
        op: OpId,
        parent: Option<OpId>,
        method: MethodId,
        #[serde(with = "serde_bytes")]
        args: Vec<u8>,
    },
    Yield {
        op: OpId,
        #[serde(with = "serde_bytes")]
        item: Vec<u8>,
    },
    Complete {
        op: OpId,
        outcome: Outcome,
    },
    Cancel {
        op: OpId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Outcome {
    Ok(#[serde(with = "serde_bytes")] Vec<u8>),
    Err(OpError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum OpError {
    Canceled,
    Unsupported,
    Denied,
}

impl fmt::Display for OpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            OpError::Canceled => "the operation was canceled",
            OpError::Unsupported => "the other side does not support the operation",
            OpError::Denied => "the other side refused the operation",
        })
    }
}

impl std::error::Error for OpError {}

impl Outcome {
    pub fn returned<T: Serialize>(value: &T) -> Self {
        Outcome::Ok(encode(value))
    }

    pub fn decode<T: DeserializeOwned>(self) -> Result<T, OpError> {
        match self {
            Outcome::Ok(bytes) => decode(&bytes).ok_or(OpError::Unsupported),
            Outcome::Err(error) => Err(error),
        }
    }
}

pub fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_stdvec(value).expect("operation values always encode")
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

pub const fn method_id(name: &str) -> MethodId {
    let bytes = name.as_bytes();
    let mut hash: u32 = 0x811c_9dc5;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Started {
    pub op: OpId,
    pub parent: Option<OpId>,
    pub method: MethodId,
    pub args: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Incoming {
    Started(Started),
    Yielded { op: OpId, item: Vec<u8> },
    Completed { op: OpId, outcome: Outcome },
    Canceled { op: OpId },
}

struct Served {
    parent: Option<OpId>,
    canceled: bool,
}

pub struct Connection {
    side: Side,
    next: OpId,
    started: BTreeMap<OpId, Option<OpId>>,
    served: BTreeMap<OpId, Served>,
    canceling: BTreeSet<OpId>,
    events: VecDeque<Incoming>,
    outbound: Vec<Vec<u8>>,
}

impl Connection {
    pub fn new(side: Side) -> Self {
        Self {
            side,
            next: 0,
            started: BTreeMap::new(),
            served: BTreeMap::new(),
            canceling: BTreeSet::new(),
            events: VecDeque::new(),
            outbound: Vec::new(),
        }
    }

    pub fn side(&self) -> Side {
        self.side
    }

    fn live(&self, op: OpId) -> bool {
        self.started.contains_key(&op) || self.served.contains_key(&op)
    }

    fn send(&mut self, frame: &Frame) {
        self.outbound.push(encode_frame(frame));
    }

    pub fn start(&mut self, parent: Option<OpId>, method: MethodId, args: Vec<u8>) -> OpId {
        self.next += 1;
        let op = self.side.bit() | self.next;
        self.started.insert(op, parent);
        self.send(&Frame::Start {
            op,
            parent,
            method,
            args,
        });
        op
    }

    pub fn cancel(&mut self, op: OpId) {
        if self.started.contains_key(&op) && self.canceling.insert(op) {
            self.send(&Frame::Cancel { op });
        }
    }

    pub fn yield_item(&mut self, op: OpId, item: Vec<u8>) {
        if self.served.contains_key(&op) {
            self.send(&Frame::Yield { op, item });
        }
    }

    pub fn complete(&mut self, op: OpId, outcome: Outcome) {
        if self.served.remove(&op).is_some() {
            self.send(&Frame::Complete { op, outcome });
            self.ended(op);
        }
    }

    pub fn is_serving(&self, op: OpId) -> bool {
        self.served.contains_key(&op)
    }

    pub fn is_pending(&self, op: OpId) -> bool {
        self.started.contains_key(&op)
    }

    fn ended(&mut self, op: OpId) {
        let children: Vec<OpId> = self
            .served
            .iter()
            .filter(|(_, served)| served.parent == Some(op) && !served.canceled)
            .map(|(child, _)| *child)
            .collect();
        for child in children {
            self.request_cancel(child);
        }
    }

    fn request_cancel(&mut self, op: OpId) {
        if let Some(served) = self.served.get_mut(&op)
            && !served.canceled
        {
            served.canceled = true;
            self.events.push_back(Incoming::Canceled { op });
        }
    }

    pub fn receive(&mut self, frame: &[u8]) -> Result<(), FrameError> {
        match decode_frame::<Frame>(frame)? {
            Frame::Start {
                op,
                parent,
                method,
                args,
            } => {
                if self.side.started(op) || self.live(op) {
                    return Ok(());
                }
                self.served.insert(
                    op,
                    Served {
                        parent,
                        canceled: false,
                    },
                );
                if parent.is_some_and(|parent| !self.live(parent)) {
                    self.complete(op, Outcome::Err(OpError::Canceled));
                    return Ok(());
                }
                self.events.push_back(Incoming::Started(Started {
                    op,
                    parent,
                    method,
                    args,
                }));
            }
            Frame::Yield { op, item } => {
                if self.started.contains_key(&op) {
                    self.events.push_back(Incoming::Yielded { op, item });
                }
            }
            Frame::Complete { op, outcome } => {
                if self.started.remove(&op).is_some() {
                    self.canceling.remove(&op);
                    self.events.push_back(Incoming::Completed { op, outcome });
                    self.ended(op);
                }
            }
            Frame::Cancel { op } => self.request_cancel(op),
        }
        Ok(())
    }

    pub fn next_event(&mut self) -> Option<Incoming> {
        self.events.pop_front()
    }

    pub fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outbound)
    }

    pub fn has_outbound(&self) -> bool {
        !self.outbound.is_empty()
    }
}

pub struct Ticket<T> {
    op: OpId,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Ticket<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Ticket<T> {}

impl<T> fmt::Debug for Ticket<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Ticket({})", self.op)
    }
}

impl<T: DeserializeOwned> Ticket<T> {
    pub fn new(op: OpId) -> Self {
        Self {
            op,
            value: PhantomData,
        }
    }

    pub fn op(self) -> OpId {
        self.op
    }

    pub fn decode(self, outcome: Outcome) -> Result<T, OpError> {
        outcome.decode()
    }
}

pub struct Reply<T> {
    op: OpId,
    value: PhantomData<fn(T)>,
}

impl<T> fmt::Debug for Reply<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Reply({})", self.op)
    }
}

impl<T> PartialEq for Reply<T> {
    fn eq(&self, other: &Self) -> bool {
        self.op == other.op
    }
}

impl<T> Eq for Reply<T> {}

impl<T: Serialize> Reply<T> {
    pub fn new(op: OpId) -> Self {
        Self {
            op,
            value: PhantomData,
        }
    }

    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn send(self, connection: &mut Connection, value: &T) {
        connection.complete(self.op, Outcome::returned(value));
    }

    pub fn fail(self, connection: &mut Connection, error: OpError) {
        connection.complete(self.op, Outcome::Err(error));
    }
}

pub struct Producer<T> {
    op: OpId,
    item: PhantomData<fn(T)>,
}

impl<T> fmt::Debug for Producer<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Producer({})", self.op)
    }
}

impl<T> PartialEq for Producer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.op == other.op
    }
}

impl<T> Eq for Producer<T> {}

impl<T: Serialize> Producer<T> {
    pub fn new(op: OpId) -> Self {
        Self {
            op,
            item: PhantomData,
        }
    }

    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn send(&self, connection: &mut Connection, item: &T) {
        connection.yield_item(self.op, encode(item));
    }

    pub fn finish(self, connection: &mut Connection) {
        connection.complete(self.op, Outcome::returned(&()));
    }

    pub fn fail(self, connection: &mut Connection, error: OpError) {
        connection.complete(self.op, Outcome::Err(error));
    }
}

#[cfg(test)]
mod tests;
