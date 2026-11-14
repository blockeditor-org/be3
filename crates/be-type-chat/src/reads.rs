use be_block_content::Root;
use be_block_content::layered::{KindVisibility, Layered, union};
use be_block_model::{Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

/// Where the account has read each chat up to, per device. An owner-only settings kind
/// (guides/settings.md, Owner only): kept apart from the chat, where it would be a read
/// receipt, readable by this account alone (administrators and the server included), and
/// synced to every device it uses.
///
/// A place is a key of its own, `chat/device/message`, so a device moving on writes a key no
/// other device has seen, and drops only keys it has seen: its own older place, and others'
/// places that are not further on. A move another device made meanwhile is a key nobody
/// dropped, so a device behind or offline never takes reading back, and the chat counts from
/// the furthest place (`ChatContent::unread`). The kind grows only with the devices still
/// ahead.
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ChatReads {
    /// Each device's last message read, keyed `chat/device/message`.
    pub read: Map<String, ObjectId>,
}

/// One device's place in a chat, and the key that holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadPlace {
    pub key: String,
    pub device: Uuid,
    pub message: ObjectId,
}

impl ChatReads {
    /// Where each device read `chat` up to.
    pub fn places(&self, chat: Uuid) -> Vec<ReadPlace> {
        let prefix = format!("{chat}/");
        self.read
            .iter()
            .filter_map(|(key, message)| {
                let rest = key.strip_prefix(&prefix)?;
                let device = rest.split('/').next()?.parse().ok()?;
                Some(ReadPlace {
                    key: key.clone(),
                    device,
                    message: *message,
                })
            })
            .collect()
    }

    /// Puts `device`'s place in `chat` at `message`, and drops the places in `seen` that no
    /// longer count: this device's older ones, and others' not further on.
    pub fn set_read(chat: Uuid, device: Uuid, message: ObjectId, seen: &[ReadPlace]) -> Edit {
        let key = format!("{chat}/{device}/{message}");
        std::iter::once(Self::READ.put(ObjectId::ROOT, &key, Some(&message)))
            .chain(
                seen.iter()
                    .filter(|place| place.key != key)
                    .map(|place| Self::READ.put(ObjectId::ROOT, &place.key, None::<&ObjectId>)),
            )
            .collect()
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
