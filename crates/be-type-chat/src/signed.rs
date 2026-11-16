//! The bytes a member's signature covers (plans/authorship.md). Each starts with a domain naming what it is, so the bytes of one
//! kind of record can never be read as another's, and then holds fixed-width fields and
//! length-prefixed strings in a fixed order: the same record always gives the same bytes,
//! whichever build writes them.

use be_block_model::{BlockRef, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A message's hash: SHA-256 over its signed bytes.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct MessageHash(#[serde(with = "serde_bytes")] pub [u8; 32]);

impl MessageHash {
    pub fn of(signed: &[u8]) -> Self {
        Self(Sha256::digest(signed).into())
    }
}

/// A message another names as coming before it (`Message::seen`, `Message::own_previous`): its
/// id, by which a reader finds it, or the closed segment that may hold it (`Segment::may_hold`),
/// and its hash, so it is that message and no other under its id.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Prior {
    pub id: ObjectId,
    pub hash: MessageHash,
}

/// What a signature covers of a body: its SHA-256.
pub(crate) fn body_hash(body: &str) -> [u8; 32] {
    Sha256::digest(body.as_bytes()).into()
}

/// What a message's signature covers of what it says (`Message::content`): a commitment to its
/// body, what it replies to and its attachments under a salt of its own, so what a deletion
/// keeps of it (`Sent`) proves who wrote it and tells nothing of what it said.
pub(crate) fn content(
    salt: &[u8; 16],
    body: &str,
    reply: Option<ObjectId>,
    attachments: &[BlockRef],
) -> [u8; 32] {
    let mut signed = Signed(Vec::new());
    signed.text("be3.chat.content.v1");
    signed.0.extend_from_slice(salt);
    signed.text(body).reply(reply).blocks(attachments);
    Sha256::digest(signed.finish()).into()
}

pub(crate) struct Signed(Vec<u8>);

impl Signed {
    pub(crate) fn new(domain: &str, chat: Uuid, message: ObjectId) -> Self {
        let mut signed = Self(Vec::new());
        signed.text(domain);
        signed.uuid(chat);
        signed.uuid(message.as_uuid());
        signed
    }

    pub(crate) fn uuid(&mut self, id: Uuid) -> &mut Self {
        self.0.extend_from_slice(id.as_bytes());
        self
    }

    pub(crate) fn time(&mut self, time: i64) -> &mut Self {
        self.0.extend_from_slice(&time.to_be_bytes());
        self
    }

    pub(crate) fn text(&mut self, text: &str) -> &mut Self {
        self.0.extend_from_slice(&(text.len() as u64).to_be_bytes());
        self.0.extend_from_slice(text.as_bytes());
        self
    }

    /// A body is signed by its hash, so a signature over a long message stays small to check
    /// against a body read elsewhere.
    pub(crate) fn body(&mut self, body: &str) -> &mut Self {
        self.digest(&body_hash(body))
    }

    /// A 32-byte digest as it is: a body's hash (`body_hash`), or a message's content
    /// (`content`).
    pub(crate) fn digest(&mut self, digest: &[u8; 32]) -> &mut Self {
        self.0.extend_from_slice(digest);
        self
    }

    pub(crate) fn reply(&mut self, reply: Option<ObjectId>) -> &mut Self {
        match reply {
            Some(reply) => {
                self.0.push(1);
                self.uuid(reply.as_uuid())
            }
            None => {
                self.0.push(0);
                self
            }
        }
    }

    pub(crate) fn seen(&mut self, seen: Option<MessageHash>) -> &mut Self {
        match seen {
            Some(seen) => {
                self.0.push(1);
                self.0.extend_from_slice(&seen.0);
            }
            None => self.0.push(0),
        }
        self
    }

    pub(crate) fn prior(&mut self, prior: Option<Prior>) -> &mut Self {
        match prior {
            Some(prior) => {
                self.0.push(1);
                self.uuid(prior.id.as_uuid());
                self.0.extend_from_slice(&prior.hash.0);
            }
            None => self.0.push(0),
        }
        self
    }

    pub(crate) fn blocks(&mut self, blocks: &[BlockRef]) -> &mut Self {
        self.0
            .extend_from_slice(&(blocks.len() as u64).to_be_bytes());
        for block in blocks {
            self.uuid(block.0);
        }
        self
    }

    pub(crate) fn count(&mut self, count: u64) -> &mut Self {
        self.0.extend_from_slice(&count.to_be_bytes());
        self
    }

    pub(crate) fn flag(&mut self, flag: bool) -> &mut Self {
        self.0.push(u8::from(flag));
        self
    }

    pub(crate) fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}
