//! A closed segment: the oldest recent messages as one part of the chat's stored form
//! (plans/chat.md, step 1d), never changed once written. The part is the messages as they were
//! sent (their edits, deletions and reactions stay in the header, keyed by message id), in a
//! fixed encoding, so the same messages always give the same bytes and hash.

use be_block_model::{BlockRef, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::chat::{Message, Segment, Signature};
use crate::signed::MessageHash;

/// A segment holds at least this many encoded bytes of messages once it is closed, unless the
/// chat has fewer: parts of about 64 to 256 KiB (a part past `LARGEST` is never closed over).
pub const SMALLEST: usize = 64 * 1024;
/// The most a segment holds, so closing it is one operation, well under what a session relays.
pub const LARGEST: usize = 256 * 1024;
/// What stays recent, at least, when a segment closes: the messages someone reading the end of
/// the chat is looking at.
pub const KEPT_RECENT: usize = 32 * 1024;

/// A message as a segment stores it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct Stored {
    id: ObjectId,
    author: Uuid,
    time: i64,
    body: String,
    reply_to: Option<ObjectId>,
    attachments: Vec<Uuid>,
    seen: Option<MessageHash>,
    #[serde(with = "serde_bytes")]
    signature: Vec<u8>,
}

impl Stored {
    fn of(id: ObjectId, message: &Message) -> Self {
        Self {
            id,
            author: message.author,
            time: message.time,
            body: message.body.clone(),
            reply_to: message.reply_to,
            attachments: message
                .attachments
                .iter()
                .map(|attachment| attachment.0)
                .collect(),
            seen: message.seen,
            signature: message.signature.0.clone(),
        }
    }

    fn message(self) -> (ObjectId, Message) {
        let message = Message {
            author: self.author,
            time: self.time,
            body: self.body,
            reply_to: self.reply_to,
            attachments: self.attachments.into_iter().map(BlockRef).collect(),
            seen: self.seen,
            signature: Signature(self.signature),
        };
        (self.id, message)
    }
}

/// The bytes a message takes in a segment.
pub fn size_of(id: ObjectId, message: &Message) -> usize {
    postcard::to_stdvec(&Stored::of(id, message)).map_or(0, |bytes| bytes.len())
}

/// A segment's bytes: `messages` in order.
pub fn encode(messages: &[(ObjectId, Message)]) -> Vec<u8> {
    let stored: Vec<Stored> = messages
        .iter()
        .map(|(id, message)| Stored::of(*id, message))
        .collect();
    postcard::to_stdvec(&stored).unwrap_or_default()
}

/// The messages in a segment's bytes, or None when they are not a segment's.
pub fn decode(bytes: &[u8]) -> Option<Vec<(ObjectId, Message)>> {
    let stored: Vec<Stored> = postcard::from_bytes(bytes).ok()?;
    Some(stored.into_iter().map(Stored::message).collect())
}

fn hash(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

/// Bits of Bloom filter per message and the number of probes: a false positive about one time in
/// a hundred.
const BITS_PER_MESSAGE: usize = 10;
const PROBES: usize = 7;

fn probes(id: ObjectId) -> [u64; PROBES] {
    let digest = Sha256::digest(id.as_uuid().as_bytes());
    let mut out = [0; PROBES];
    for (probe, slot) in out.iter_mut().enumerate() {
        let mut word = [0; 4];
        word.copy_from_slice(&digest[probe * 4..probe * 4 + 4]);
        *slot = u64::from(u32::from_be_bytes(word));
    }
    out
}

impl Segment {
    /// The index entry for `messages` (oldest first, not empty) written as `bytes`.
    pub fn of(messages: &[(ObjectId, Message)], bytes: &[u8]) -> Self {
        let mut members = vec![0; (messages.len() * BITS_PER_MESSAGE).div_ceil(8).max(1)];
        let bits = members.len() as u64 * 8;
        for (id, _) in messages {
            for probe in probes(*id) {
                let bit = probe % bits;
                members[usize::try_from(bit / 8).unwrap_or(0)] |= 1 << (bit % 8);
            }
        }
        Self {
            first: messages.first().map(|(id, _)| *id).unwrap_or_default(),
            last: messages.last().map(|(id, _)| *id).unwrap_or_default(),
            from: messages
                .iter()
                .map(|(_, message)| message.time)
                .min()
                .unwrap_or(0),
            to: messages
                .iter()
                .map(|(_, message)| message.time)
                .max()
                .unwrap_or(0),
            count: messages.len() as u64,
            length: bytes.len() as u64,
            hash: hash(bytes),
            members,
        }
    }

    /// Whether `message` may be in this segment: always when it is, rarely when it is not.
    pub fn may_hold(&self, message: ObjectId) -> bool {
        let bits = self.members.len() as u64 * 8;
        bits > 0
            && probes(message).into_iter().all(|probe| {
                let bit = probe % bits;
                self.members[usize::try_from(bit / 8).unwrap_or(0)] & (1 << (bit % 8)) != 0
            })
    }

    /// The messages in `bytes` if they are this segment's part (its length and hash match).
    pub fn read(&self, bytes: &[u8]) -> Option<Vec<(ObjectId, Message)>> {
        if bytes.len() as u64 != self.length || hash(bytes) != self.hash {
            return None;
        }
        decode(bytes)
    }
}
