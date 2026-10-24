use be_session::SessionMessage;
use block_type_api::SessionTag;
use uuid::Uuid;

// Peers speak be-session's SessionMessage exactly as be-client's Live does, so
// a replica and a Live peer of one build share a session. A replica says which
// build it is on two channels a Live peer passes over: the tag ahead of each
// Catchup, and the owner's answer to a tag that is not its own.
const TAG_CHANNEL: Uuid = Uuid::from_u128(0x6be3_7a90_1d1c_4c5e_9a1b_0b1e_7a9c_0001);
const INCOMPATIBLE_CHANNEL: Uuid = Uuid::from_u128(0x6be3_7a90_1d1c_4c5e_9a1b_0b1e_7a9c_0002);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Wire {
    Session(SessionMessage),
    Tag(SessionTag),
    Incompatible,
}

impl Wire {
    pub fn encode(&self) -> Vec<u8> {
        let message = match self {
            Self::Session(message) => message.clone(),
            Self::Tag(tag) => SessionMessage::Channel {
                channel: TAG_CHANNEL,
                payload: tag.0.to_vec(),
            },
            Self::Incompatible => SessionMessage::Channel {
                channel: INCOMPATIBLE_CHANNEL,
                payload: Vec::new(),
            },
        };
        be_protocol::encode(&message).expect("session messages always encode")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        Some(match be_protocol::decode::<SessionMessage>(bytes).ok()? {
            SessionMessage::Channel { channel, payload } if channel == TAG_CHANNEL => {
                Self::Tag(SessionTag(payload.try_into().ok()?))
            }
            SessionMessage::Channel { channel, .. } if channel == INCOMPATIBLE_CHANNEL => {
                Self::Incompatible
            }
            message => Self::Session(message),
        })
    }
}
