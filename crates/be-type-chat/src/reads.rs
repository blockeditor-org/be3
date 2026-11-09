use be_block::Root;
use be_block::settings::{KindVisibility, Layered, union};
use be_block_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where the account has read each chat up to, by the chat's id. An owner-only settings kind
/// (guides/settings.md, Owner only): kept apart from the chat, where it would be a read
/// receipt, readable by this account alone (administrators and the server included), and the
/// same on every device it uses.
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ChatReads {
    pub chats: Map<String, ReadUpTo>,
}

/// The newest message read, and its claimed time for when that message is gone.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct ReadUpTo {
    pub message: ObjectId,
    pub time: i64,
}

impl ChatReads {
    pub fn read_up_to(&self, chat: Uuid) -> Option<&ReadUpTo> {
        self.chats.get(&chat.to_string())
    }

    pub fn set_read(chat: Uuid, read: &ReadUpTo) -> Edit {
        Self::CHATS
            .put(ObjectId::ROOT, &chat.to_string(), Some(read))
            .into()
    }
}

impl Layered for ChatReads {
    const NAME: &'static str = "Chats read";
    const VISIBILITY: KindVisibility = KindVisibility::Owner;

    fn over(&self, lower: &Self) -> Self {
        Self {
            chats: union(&self.chats, &lower.chats),
        }
    }
}

impl Root for ChatReads {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6174_2d72_6561_6473_0000_0000_0001);
}

pub type ChatReadsContent = Document<ChatReads>;
