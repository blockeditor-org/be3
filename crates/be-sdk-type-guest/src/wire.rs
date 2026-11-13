use be_block_session::SessionMessage;
use be_sdk_type_api::{RelayKind, SessionTag};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Wire {
    Session(SessionMessage),
    Tag(SessionTag),
    Incompatible,
}

impl Wire {
    pub fn encode(&self) -> Vec<u8> {
        be_block_protocol::encode(self).expect("session messages always encode")
    }

    /// What the server checks before relaying this: who may send it.
    pub fn kind(&self) -> RelayKind {
        let Wire::Session(message) = self else {
            return RelayKind::Peer;
        };
        match message {
            SessionMessage::Catchup { .. }
            | SessionMessage::Presence { .. }
            | SessionMessage::Channel { .. } => RelayKind::Peer,
            SessionMessage::Submit { .. }
            | SessionMessage::Kept { .. }
            | SessionMessage::Accepted { .. }
            | SessionMessage::Snapshot { .. }
            | SessionMessage::Sealed { .. }
            | SessionMessage::Replaced { .. }
            | SessionMessage::Saved { .. }
            | SessionMessage::Resend => RelayKind::Edit,
        }
    }

    /// Whether a message the server relayed as `kind` may say this: what the server checked
    /// covers what it asks for. A viewer could label a `Submit` as presence; it is dropped.
    pub fn sent_as(&self, kind: RelayKind) -> bool {
        kind == RelayKind::Edit || self.kind() == RelayKind::Peer
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_block_protocol::decode(bytes).ok()
    }
}
