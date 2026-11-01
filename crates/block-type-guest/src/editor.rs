use std::collections::VecDeque;

use be_block::{LiveEdit, Settling, Touched};
use block_type_api::ClientId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Gesture {
    Begin,
    Commit,
    Cancel,
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub content_type: Uuid,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub session: Vec<u8>,
    #[serde(
        serialize_with = "be_ops::wire::serialize::<be_ops::wire::List<be_ops::wire::Bytes>, _, _>",
        deserialize_with = "be_ops::wire::deserialize::<be_ops::wire::List<be_ops::wire::Bytes>, _, _>"
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
        serialize_with = "be_ops::wire::serialize::<be_ops::wire::Optional<be_ops::wire::List<be_ops::wire::Bytes>>, _, _>",
        deserialize_with = "be_ops::wire::deserialize::<be_ops::wire::Optional<be_ops::wire::List<be_ops::wire::Bytes>>, _, _>"
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
        block_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        block_type_api::decode_frame(frame).ok()
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
        block_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        block_type_api::decode_frame(frame).ok()
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
