use be_block_session::SessionMessage;
use be_sdk_type_api::SessionTag;
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

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        be_block_protocol::decode(bytes).ok()
    }
}
