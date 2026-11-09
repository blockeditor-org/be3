use be_block::Root;
use be_block::settings::{KindVisibility, Layered, union};
use be_block_model::{Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

/// Where the account has read each chat up to, per device. An owner-only settings kind
/// (guides/settings.md, Owner only): kept apart from the chat, where it would be a read
/// receipt, readable by this account alone (administrators and the server included), and
/// synced to every device it uses.
///
/// Each device writes only its own entry, and only to move it on, so a device behind or offline
/// never takes another's reading back: the chat counts from the furthest entry
/// (`ChatContent::unread`), which `Map`'s later write winning per key could not give. A device
/// moving on drops the entries not further than its own.
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ChatReads {
    /// The last message read, by the chat's id and the device's client id, as `chat/device`.
    pub read: Map<String, ObjectId>,
}

fn key(chat: Uuid, device: Uuid) -> String {
    format!("{chat}/{device}")
}

impl ChatReads {
    /// Where each device read `chat` up to, by the device's client id.
    pub fn places(&self, chat: Uuid) -> Vec<(Uuid, ObjectId)> {
        let prefix = format!("{chat}/");
        self.read
            .iter()
            .filter_map(|(key, message)| {
                let device = key.strip_prefix(&prefix)?.parse().ok()?;
                Some((device, *message))
            })
            .collect()
    }

    /// Where `device` read `chat` up to.
    pub fn read_here(&self, chat: Uuid, device: Uuid) -> Option<ObjectId> {
        self.read.get(&key(chat, device)).copied()
    }

    /// Moves `device`'s place in `chat` to `message`, and drops the places of the devices in
    /// `behind`, which are not further on and so no longer count: the kind grows with the
    /// devices still ahead, not with every device that ever read the chat.
    pub fn set_read(chat: Uuid, device: Uuid, message: ObjectId, behind: &[Uuid]) -> Edit {
        std::iter::once(Self::READ.put(ObjectId::ROOT, &key(chat, device), Some(&message)))
            .chain(
                behind
                    .iter()
                    .filter(|other| **other != device)
                    .map(|other| {
                        Self::READ.put(ObjectId::ROOT, &key(chat, *other), None::<&ObjectId>)
                    }),
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
