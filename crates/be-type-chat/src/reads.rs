use std::collections::BTreeMap;

use be_block_content::layered::{KindVisibility, Layered, union};
use be_block_content::{LogItem, Root};
use be_block_model::{Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

use crate::ChatLog;

/// Where the account has read each chat up to, per device: the place in the chat's log of the
/// last entry read. An owner-only settings kind (guides/settings.md, Owner only): kept apart
/// from the chat, where it would be a read receipt, readable by this account alone
/// (administrators and the server included), and synced to every device it uses.
///
/// Each device writes its own key, `chat/device`, so devices moving on at once never meet,
/// and the chat counts from the furthest place any of them read (`ChatLog::unread`): a device
/// behind or offline never takes reading back.
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ChatReads {
    /// Each device's last place read, keyed `chat/device`.
    pub read: Map<String, u64>,
}

impl ChatReads {
    /// The furthest place any device read `chat` up to.
    pub fn place(&self, chat: Uuid) -> Option<u64> {
        let prefix = format!("{chat}/");
        self.read
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, place)| *place)
            .max()
    }

    /// Puts `device`'s place in `chat` at `place`.
    pub fn set_read(chat: Uuid, device: Uuid, place: u64) -> Edit {
        Self::READ
            .put(ObjectId::ROOT, &format!("{chat}/{device}"), Some(&place))
            .into()
    }
}

impl Layered for ChatReads {
    const NAME: &'static str = "Chats read";
    const VISIBILITY: KindVisibility = KindVisibility::Owner;

    fn over(&self, lower: &Self) -> Self {
        Self {
            read: union(&self.read, &lower.read),
        }
    }
}

impl Root for ChatReads {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6174_2d72_6561_6473_0000_0000_0001);
}

pub type ChatReadsContent = Document<ChatReads>;

impl be_block_content::Unread for crate::ChatContent {
    type Reads = ChatReads;
    /// The furthest place read.
    type Place = Option<u64>;

    fn places(reads: &ChatReads) -> Vec<(Uuid, Option<u64>)> {
        let mut places: BTreeMap<Uuid, u64> = BTreeMap::new();
        for (key, place) in reads.read.iter() {
            if let Some(chat) = key.split('/').next().and_then(|chat| chat.parse().ok()) {
                let furthest = places.entry(chat).or_insert(*place);
                *furthest = (*furthest).max(*place);
            }
        }
        places
            .into_iter()
            .map(|(chat, place)| (chat, Some(place)))
            .collect()
    }

    /// The messages others sent among the log's latest entries since the place read
    /// (`ChatLog::unread`).
    fn count_unread(&self, place: &Option<u64>, log: &[LogItem], me: Uuid) -> u32 {
        u32::try_from(ChatLog::of(log).unread(*place, me)).unwrap_or(u32::MAX)
    }
}
