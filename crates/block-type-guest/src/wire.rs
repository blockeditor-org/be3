use be_session::SessionMessage;
use block_type_api::SessionTag;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Wire {
    Session(SessionMessage),
    Tag(SessionTag),
    Incompatible,
}

impl Wire {
    pub fn encode(&self) -> Vec<u8> {
        be_protocol::encode(self).expect("session messages always encode")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_protocol::decode(bytes).ok()
    }
}
