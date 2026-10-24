use be_session::SessionMessage;
use block_type_api::SessionTag;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum Wire {
    Session(SessionMessage),
    Greeting { tag: SessionTag, since: u64 },
    Incompatible,
}

impl Wire {
    pub(crate) fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("session messages always encode")
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }
}
