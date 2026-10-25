use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    marker::PhantomData,
};

use block_guest_abi::direct;
use serde::Deserialize;
pub use serde::{Serialize, de::DeserializeOwned};

pub mod guest;

#[doc(hidden)]
pub use block_guest_abi as __abi;

pub type OpId = u64;

pub type MethodId = u32;

pub const GUEST_BIT: OpId = 1 << 63;

pub const STREAM_WINDOW: u32 = 16;

pub const STREAM_WINDOW_BYTES: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    items: u32,
    bytes: i64,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            items: STREAM_WINDOW,
            bytes: STREAM_WINDOW_BYTES as i64,
        }
    }
}

impl Window {
    pub fn open(&self) -> bool {
        self.items > 0 && self.bytes > 0
    }

    pub fn take(&mut self, bytes: usize) {
        self.items = self.items.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(bytes as i64);
    }

    pub fn grant(&mut self, items: u32, bytes: u64) {
        self.items = self.items.saturating_add(items);
        self.bytes = self.bytes.saturating_add(bytes as i64);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WouldBlock;

impl fmt::Display for WouldBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the stream's consumer has granted no room for another item")
    }
}

impl std::error::Error for WouldBlock {}

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
    Credit {
        op: OpId,
        items: u32,
        bytes: u64,
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
pub struct Called {
    pub name: String,
    pub parent: Option<OpId>,
    pub answered: bool,
    pub args: Vec<u8>,
}

impl Called {
    pub fn method(&self) -> MethodId {
        method_id(&self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Incoming {
    Started(Started),
    Called(Called),
    Yielded { op: OpId, item: Vec<u8> },
    Completed { op: OpId, outcome: Outcome },
    Canceled { op: OpId },
    Credited { op: OpId },
}

struct Served {
    parent: Option<OpId>,
    canceled: bool,
    window: Window,
}

impl Served {
    fn new(parent: Option<OpId>) -> Self {
        Self {
            parent,
            canceled: false,
            window: Window::default(),
        }
    }
}

pub struct Connection {
    side: Side,
    next: OpId,
    started: BTreeMap<OpId, Option<OpId>>,
    served: BTreeMap<OpId, Served>,
    canceling: BTreeSet<OpId>,
    direct: VecDeque<OpId>,
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
            direct: VecDeque::new(),
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

    pub fn call_direct(&mut self, parent: Option<OpId>, name: &str, args: Vec<u8>) -> OpId {
        self.next += 1;
        let op = self.side.bit() | self.next;
        self.direct.push_back(op);
        self.outbound.push(direct::call(name, parent, true, &args));
        op
    }

    pub fn notify_direct(&mut self, parent: Option<OpId>, name: &str, args: &[u8]) {
        self.outbound.push(direct::call(name, parent, false, args));
    }

    pub fn return_direct(&mut self, outcome: &Outcome) {
        self.outbound.push(direct::returned(&encode(outcome)));
    }

    pub fn cancel(&mut self, op: OpId) {
        if self.started.contains_key(&op) && self.canceling.insert(op) {
            self.send(&Frame::Cancel { op });
        }
    }

    pub fn ready(&self, op: OpId) -> bool {
        self.served
            .get(&op)
            .is_none_or(|served| served.window.open())
    }

    pub fn yield_item(&mut self, op: OpId, item: &[u8]) -> Result<(), WouldBlock> {
        let Some(served) = self.served.get_mut(&op) else {
            return Ok(());
        };
        if !served.window.open() {
            return Err(WouldBlock);
        }
        served.window.take(item.len());
        self.send(&Frame::Yield {
            op,
            item: item.to_vec(),
        });
        Ok(())
    }

    pub fn credit(&mut self, op: OpId, items: u32, bytes: u64) {
        if (items > 0 || bytes > 0)
            && self.started.contains_key(&op)
            && !self.canceling.contains(&op)
        {
            self.send(&Frame::Credit { op, items, bytes });
        }
    }

    pub fn complete(&mut self, op: OpId, outcome: Outcome) {
        if self.served.remove(&op).is_none() {
            return;
        }
        self.send(&Frame::Complete { op, outcome });
        self.ended(op);
    }

    fn granted(&mut self, op: OpId, items: u32, bytes: u64) {
        let Some(served) = self.served.get_mut(&op) else {
            return;
        };
        let was_open = served.window.open();
        served.window.grant(items, bytes);
        if !was_open && served.window.open() && !served.canceled {
            self.events.push_back(Incoming::Credited { op });
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

    pub fn receive(&mut self, item: &[u8]) -> Result<(), FrameError> {
        let malformed = || FrameError::Malformed("a direct call did not decode".to_owned());
        let frame = match direct::decode(item).ok_or_else(malformed)? {
            direct::Item::Frame(frame) => frame,
            direct::Item::Call(call) => {
                self.events.push_back(Incoming::Called(Called {
                    name: call.name.to_owned(),
                    parent: call.parent,
                    answered: call.answered,
                    args: call.args.to_vec(),
                }));
                return Ok(());
            }
            direct::Item::Returned(result) => {
                let op = self.direct.pop_front().ok_or_else(malformed)?;
                let outcome = match result.is_empty() {
                    true => Outcome::Ok(Vec::new()),
                    false => decode(result).ok_or_else(malformed)?,
                };
                self.events.push_back(Incoming::Completed { op, outcome });
                return Ok(());
            }
        };
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
                self.served.insert(op, Served::new(parent));
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
            Frame::Credit { op, items, bytes } => self.granted(op, items, bytes),
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

    pub fn send(&self, connection: &mut Connection, item: &T) -> Result<(), WouldBlock> {
        if !connection.ready(self.op) {
            return Err(WouldBlock);
        }
        connection.yield_item(self.op, &encode(item))
    }

    pub fn ready(&self, connection: &Connection) -> bool {
        connection.ready(self.op)
    }

    pub fn finish(self, connection: &mut Connection) {
        connection.complete(self.op, Outcome::returned(&()));
    }

    pub fn fail(self, connection: &mut Connection, error: OpError) {
        connection.complete(self.op, Outcome::Err(error));
    }
}

pub struct Latest<T> {
    producer: Producer<T>,
    behind: Option<T>,
}

impl<T> fmt::Debug for Latest<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Latest({})", self.producer.op)
    }
}

impl<T: Serialize> Latest<T> {
    pub fn new(producer: Producer<T>) -> Self {
        Self {
            producer,
            behind: None,
        }
    }

    pub fn op(&self) -> OpId {
        self.producer.op
    }

    pub fn send(&mut self, connection: &mut Connection, value: T) {
        self.behind = match self.producer.send(connection, &value) {
            Ok(()) => None,
            Err(WouldBlock) => Some(value),
        };
    }

    pub fn credited(&mut self, connection: &mut Connection, op: OpId) -> bool {
        if op != self.op() {
            return false;
        }
        if let Some(value) = self.behind.take() {
            self.send(connection, value);
        }
        true
    }

    pub fn finish(self, connection: &mut Connection) {
        self.producer.finish(connection);
    }
}

#[cfg(test)]
mod tests;
