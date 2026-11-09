use std::collections::VecDeque;

use be_block::{LiveEdit, Settling, Touched};
use be_sdk_type_api::ClientId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Gesture {
    Begin,
    Commit,
    Cancel,
    /// Ends the current undo step without opening a gesture, so the next edit starts one of
    /// its own however soon it comes: a text editor's paste, or typing after a caret jump.
    Break,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FromEditor {
    Operate(#[serde(with = "serde_bytes")] Vec<u8>),
    Gesture(Gesture),
    Replace(#[serde(with = "serde_bytes")] Vec<u8>),
    ShowPresence {
        kind: Uuid,
        #[serde(with = "serde_bytes")]
        value: Option<Vec<u8>>,
    },
    SendToPeers {
        to: Option<ClientId>,
        channel: Uuid,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    Resend,
    /// Asks for `length` bytes of the block's stored form from `offset`, as the replica holds
    /// it: what a type that loads only its first part left in the store comes this way. The
    /// replica answers `ToEditor::Range` with the same `token`.
    Read {
        token: u64,
        offset: u64,
        length: u64,
    },
    /// Asks the replica to put `bytes` as a part of the block (`host.put-part`), so an
    /// operation can name it by its hash instead of carrying it. The replica answers
    /// `ToEditor::PartPut` with the same `token` once the part is uploaded.
    PutPart {
        token: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub content_type: Uuid,
    /// `encode()`: for a type that loads only its first part (`be_block::Loads::FirstPart`),
    /// only that part's bytes, the rest read with `FromEditor::Read`.
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    /// The whole stored form's length.
    pub length: u64,
    #[serde(with = "serde_bytes")]
    pub session: Vec<u8>,
    #[serde(
        serialize_with = "be_wasm_ops::wire::serialize::<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>, _, _>",
        deserialize_with = "be_wasm_ops::wire::deserialize::<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>, _, _>"
    )]
    pub unsettled: Vec<Vec<u8>>,
    pub applied: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorOperation {
    #[serde(with = "serde_bytes")]
    pub operation: Vec<u8>,
    pub mine: bool,
    pub unsettled: u64,
    #[serde(
        serialize_with = "be_wasm_ops::wire::serialize::<be_wasm_ops::wire::Optional<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>>, _, _>",
        deserialize_with = "be_wasm_ops::wire::deserialize::<be_wasm_ops::wire::Optional<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>>, _, _>"
    )]
    pub beneath: Option<Vec<Vec<u8>>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PeerPresence {
    pub client: ClientId,
    pub kind: Uuid,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PeerMessage {
    pub from: ClientId,
    pub channel: Uuid,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ToEditor {
    Snapshot(Snapshot),
    Operations(Vec<EditorOperation>),
    Presence(Vec<PeerPresence>),
    PeerMessages(Vec<PeerMessage>),
    Range {
        token: u64,
        #[serde(with = "range_result")]
        result: Result<Vec<u8>, String>,
    },
    PartPut {
        token: u64,
        result: Result<be_block::PartRef, String>,
    },
}

mod range_result {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize, Serialize)]
    enum Read {
        Ok(#[serde(with = "serde_bytes")] Vec<u8>),
        Err(String),
    }

    pub fn serialize<S: Serializer>(
        result: &Result<Vec<u8>, String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match result {
            Ok(bytes) => Read::Ok(bytes.clone()),
            Err(error) => Read::Err(error.clone()),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Result<Vec<u8>, String>, D::Error> {
        Ok(match Read::deserialize(deserializer)? {
            Read::Ok(bytes) => Ok(bytes),
            Read::Err(error) => Err(error),
        })
    }
}

fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_stdvec(message).expect("editor messages always encode")
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

impl FromEditor {
    pub fn encode(&self) -> Vec<u8> {
        encode(self)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        decode(bytes)
    }

    pub fn frame(&self) -> Vec<u8> {
        be_sdk_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        be_sdk_type_api::decode_frame(frame).ok()
    }
}

impl ToEditor {
    pub fn encode(&self) -> Vec<u8> {
        encode(self)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        decode(bytes)
    }

    pub fn frame(&self) -> Vec<u8> {
        be_sdk_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        be_sdk_type_api::decode_frame(frame).ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Projected<Op> {
    Applied(Op),
    Rebuilt,
}

pub struct EditorMirror<C: LiveEdit> {
    confirmed: Option<Settling<C>>,
    visible: C,
    pending: VecDeque<C::Op>,
    acknowledged: u64,
    resyncing: bool,
    touched: Vec<Touched>,
    presence: Vec<PeerPresence>,
    peer_messages: Vec<PeerMessage>,
    revision: u64,
    projected: Option<Vec<Projected<C::Op>>>,
    length: u64,
    snapshots: u64,
    reads: u64,
    ranges: Vec<(u64, Result<Vec<u8>, String>)>,
    puts: Vec<(u64, Result<be_block::PartRef, String>)>,
}

impl<C: LiveEdit + Clone + Default> Default for EditorMirror<C> {
    fn default() -> Self {
        Self {
            confirmed: None,
            visible: C::default(),
            pending: VecDeque::new(),
            acknowledged: 0,
            resyncing: false,
            touched: Vec::new(),
            presence: Vec::new(),
            peer_messages: Vec::new(),
            revision: 0,
            projected: None,
            length: 0,
            snapshots: 0,
            reads: 0,
            ranges: Vec::new(),
            puts: Vec::new(),
        }
    }
}

impl<C: LiveEdit + Clone + Default> EditorMirror<C> {
    pub fn content(&self) -> Option<&C> {
        self.confirmed.as_ref().map(|_| &self.visible)
    }

    pub fn visible(&self) -> &C {
        &self.visible
    }

    pub fn is_loaded(&self) -> bool {
        self.confirmed.is_some()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn record(&mut self) {
        self.projected.get_or_insert_with(Vec::new);
    }

    pub fn stop_recording(&mut self) {
        self.projected = None;
    }

    pub fn take_projected(&mut self) -> Vec<Projected<C::Op>> {
        self.projected
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    fn changed(&mut self, projected: Projected<C::Op>) {
        self.revision += 1;
        if let Some(log) = &mut self.projected {
            log.push(projected);
        }
    }

    /// The whole stored form's length as of the last snapshot, of which a type that loads
    /// only its first part holds the start.
    pub fn stored_length(&self) -> u64 {
        self.length
    }

    /// How many snapshots the mirror took: what it holds of a type that loads only its first
    /// part changes only with one, so a reader of the rest reads again when this moves.
    pub fn snapshots(&self) -> u64 {
        self.snapshots
    }

    /// Asks the replica for a range of the stored form; its answer comes back from `receive`
    /// and is taken with `take_ranges` under the token this returns.
    pub fn read_range(&mut self, offset: u64, length: u64) -> (u64, FromEditor) {
        self.reads += 1;
        let token = self.reads;
        (
            token,
            FromEditor::Read {
                token,
                offset,
                length,
            },
        )
    }

    pub fn take_ranges(&mut self) -> Vec<(u64, Result<Vec<u8>, String>)> {
        std::mem::take(&mut self.ranges)
    }

    /// Asks the replica to put `bytes` as a part of the block; the part's ref comes back from
    /// `receive` and is taken with `take_parts_put` under the token this returns, to put in an
    /// operation's payload.
    pub fn put_part(&mut self, bytes: Vec<u8>) -> (u64, FromEditor) {
        self.reads += 1;
        let token = self.reads;
        (token, FromEditor::PutPart { token, bytes })
    }

    pub fn take_parts_put(&mut self) -> Vec<(u64, Result<be_block::PartRef, String>)> {
        std::mem::take(&mut self.puts)
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn presence(&self) -> &[PeerPresence] {
        &self.presence
    }

    pub fn take_peer_messages(&mut self) -> Vec<PeerMessage> {
        std::mem::take(&mut self.peer_messages)
    }

    pub fn take_touched(&mut self) -> Vec<Touched> {
        std::mem::take(&mut self.touched)
    }

    pub fn operate(&mut self, operation: C::Op) -> Option<FromEditor> {
        self.confirmed.as_ref()?;
        self.visible.apply_touching(&operation, &mut self.touched);
        self.revision += 1;
        let message = FromEditor::Operate(C::encode_operation(&operation));
        self.pending.push_back(operation);
        Some(message)
    }

    pub fn receive(&mut self, message: ToEditor) -> Option<FromEditor> {
        match message {
            ToEditor::Snapshot(snapshot) => {
                self.adopt(snapshot);
                None
            }
            ToEditor::Operations(operations) => {
                if self.confirmed.is_none() || self.resyncing {
                    return None;
                }
                for operation in operations {
                    if !self.take(operation) {
                        self.resyncing = true;
                        return Some(FromEditor::Resend);
                    }
                }
                None
            }
            ToEditor::Presence(presence) => {
                self.presence = presence;
                None
            }
            ToEditor::PeerMessages(messages) => {
                self.peer_messages.extend(messages);
                None
            }
            ToEditor::Range { token, result } => {
                self.ranges.push((token, result));
                None
            }
            ToEditor::PartPut { token, result } => {
                self.puts.push((token, result));
                None
            }
        }
    }

    fn adopt(&mut self, snapshot: Snapshot) {
        if snapshot.content_type != C::CONTENT_TYPE {
            return;
        }
        let Ok(mut settled) = C::decode(&snapshot.bytes) else {
            return;
        };
        if settled.adopt_session_state(&snapshot.session).is_err() {
            return;
        }
        let Ok(unsettled) = snapshot
            .unsettled
            .iter()
            .map(|operation| C::decode_operation(operation))
            .collect::<Result<Vec<_>, _>>()
        else {
            return;
        };
        let taken = snapshot.applied.saturating_sub(self.acknowledged);
        self.acknowledged = snapshot.applied;
        for _ in 0..taken.min(self.pending.len() as u64) {
            self.pending.pop_front();
        }
        self.confirmed = Some(Settling::new(settled, unsettled));
        self.length = snapshot.length;
        self.snapshots += 1;
        self.resyncing = false;
        self.rebuild();
    }

    fn take(&mut self, landed: EditorOperation) -> bool {
        let Ok(operation) = C::decode_operation(&landed.operation) else {
            return false;
        };
        let Some(confirmed) = self.confirmed.as_mut() else {
            return false;
        };
        if let Some(local) = landed.beneath {
            let Ok(local) = local
                .iter()
                .map(|operation| C::decode_operation(operation))
                .collect::<Result<Vec<_>, _>>()
            else {
                return false;
            };
            let landed = confirmed.beneath(
                &mut self.visible,
                &operation,
                landed.unsettled,
                local,
                self.pending.iter().cloned(),
                &mut self.touched,
            );
            if landed {
                self.changed(Projected::Rebuilt);
            }
            return landed;
        }
        if landed.mine {
            self.pending.pop_front();
            self.acknowledged += 1;
            confirmed.push(operation, landed.unsettled);
            return true;
        }
        if self.pending.is_empty() {
            self.visible.apply_touching(&operation, &mut self.touched);
            confirmed.push(operation.clone(), landed.unsettled);
            self.changed(Projected::Applied(operation));
            return true;
        }
        if landed.unsettled == 0 {
            confirmed.settle_to(0);
            confirmed.beneath(
                &mut self.visible,
                &operation,
                0,
                Vec::new(),
                self.pending.iter().cloned(),
                &mut self.touched,
            );
            self.changed(Projected::Rebuilt);
            return true;
        }
        confirmed.push(operation, landed.unsettled);
        self.rebuild();
        true
    }

    fn rebuild(&mut self) {
        let Some(confirmed) = &self.confirmed else {
            return;
        };
        let mut visible = confirmed.visible();
        for operation in &self.pending {
            visible.apply(operation);
        }
        self.visible = visible;
        self.touched.push(Touched::Everything);
        self.changed(Projected::Rebuilt);
    }
}
